"""Read-only calendar sync: Google (REST) + iCloud (CalDAV) → SQLite cache.

Both sources are optional — each syncs only when its credentials are present.
A background task polls every `calendar_poll_seconds`.
"""

import asyncio
import datetime
import logging

from .config import config
from .db import get_db

log = logging.getLogger("tally.calendar")

WINDOW_DAYS = 14

# Last sync_ics failures, surfaced on the settings page
ics_errors: list[str] = []


def _upsert(events: list[dict], source: str) -> None:
    with get_db() as conn:
        conn.execute("DELETE FROM events WHERE source = ?", (source,))
        conn.executemany(
            "INSERT OR REPLACE INTO events "
            "(uid, source, calendar, title, start_utc, end_utc, all_day, location) "
            "VALUES (:uid, :source, :calendar, :title, :start_utc, :end_utc, :all_day, :location)",
            [{**e, "source": source} for e in events],
        )


def sync_icloud() -> int:
    if not (config.icloud_username and config.icloud_app_password):
        return 0
    import caldav

    now = datetime.datetime.now(datetime.timezone.utc)
    end = now + datetime.timedelta(days=WINDOW_DAYS)
    events = []
    with caldav.DAVClient(
        url="https://caldav.icloud.com/",
        username=config.icloud_username,
        password=config.icloud_app_password,
    ) as client:
        for cal in client.principal().calendars():
            try:
                found = cal.search(start=now, end=end, event=True, expand=True)
            except Exception as exc:  # some iCloud calendars (e.g. subscriptions) 403
                log.warning("icloud calendar %s failed: %s", cal, exc)
                continue
            for ev in found:
                comp = ev.icalendar_component
                start = comp.get("dtstart").dt
                dtend = comp.get("dtend")
                all_day = not isinstance(start, datetime.datetime)
                events.append({
                    "uid": f"icloud:{comp.get('uid')}:{start.isoformat()}",
                    "calendar": str(getattr(cal, "name", "") or ""),
                    "title": str(comp.get("summary", "")),
                    "start_utc": _to_utc(start),
                    "end_utc": _to_utc(dtend.dt) if dtend else None,
                    "all_day": int(all_day),
                    "location": str(comp.get("location", "") or ""),
                })
    _upsert(events, "icloud")
    return len(events)


def sync_ics() -> int:
    """Secret iCal feed URLs (e.g. Google Calendar's 'Secret address in iCal
    format') — read-only, no OAuth. One URL per line in settings."""
    ics_errors.clear()
    urls = [u.strip() for u in config.ics_urls.splitlines() if u.strip()]
    if not urls:
        _upsert([], "ics")
        return 0
    import icalendar
    import recurring_ical_events
    import requests

    now = datetime.datetime.now(datetime.timezone.utc)
    start = now - datetime.timedelta(days=1)
    end = now + datetime.timedelta(days=WINDOW_DAYS)
    events = []
    for i, url in enumerate(urls):
        try:
            resp = requests.get(url, timeout=30)
            resp.raise_for_status()
            cal = icalendar.Calendar.from_ical(resp.content)
        except Exception as exc:  # bad URL / revoked secret address / parse error
            log.warning("ics feed %d failed: %s", i + 1, exc)
            ics_errors.append(f"feed {i + 1}: {str(exc)[:100]}")
            continue
        name = str(cal.get("X-WR-CALNAME", "") or f"feed {i + 1}")
        for comp in recurring_ical_events.of(cal).between(start, end):
            dtstart = comp.get("dtstart").dt
            dtend = comp.get("dtend")
            events.append({
                "uid": f"ics:{i}:{comp.get('uid')}:{_to_utc(dtstart)}",
                "calendar": name,
                "title": str(comp.get("summary", "(no title)")),
                "start_utc": _to_utc(dtstart),
                "end_utc": _to_utc(dtend.dt) if dtend else None,
                "all_day": int(not isinstance(dtstart, datetime.datetime)),
                "location": str(comp.get("location", "") or ""),
            })
    _upsert(events, "ics")
    return len(events)


def sync_google() -> int:
    if not config.google_token.exists():
        return 0
    from google.auth.transport.requests import Request
    from google.oauth2.credentials import Credentials
    from googleapiclient.discovery import build

    creds = Credentials.from_authorized_user_file(str(config.google_token))
    if creds.expired and creds.refresh_token:
        creds.refresh(Request())
        config.google_token.write_text(creds.to_json())

    service = build("calendar", "v3", credentials=creds, cache_discovery=False)
    now = datetime.datetime.now(datetime.timezone.utc)
    end = now + datetime.timedelta(days=WINDOW_DAYS)
    events = []
    for cal in service.calendarList().list().execute().get("items", []):
        if cal.get("selected") is False:
            continue
        resp = service.events().list(
            calendarId=cal["id"],
            timeMin=now.isoformat(),
            timeMax=end.isoformat(),
            singleEvents=True,
            orderBy="startTime",
            maxResults=250,
        ).execute()
        for ev in resp.get("items", []):
            start = ev["start"].get("dateTime") or ev["start"].get("date")
            endt = ev["end"].get("dateTime") or ev["end"].get("date")
            events.append({
                "uid": f"google:{ev['id']}",
                "calendar": cal.get("summaryOverride") or cal.get("summary", ""),
                "title": ev.get("summary", "(no title)"),
                "start_utc": start,
                "end_utc": endt,
                "all_day": int("date" in ev["start"]),
                "location": ev.get("location", ""),
            })
    _upsert(events, "google")
    return len(events)


def _to_utc(dt) -> str:
    if isinstance(dt, datetime.datetime):
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=datetime.timezone.utc)
        return dt.astimezone(datetime.timezone.utc).isoformat()
    return dt.isoformat()  # date (all-day)


def get_agenda(days: int = 1) -> list[dict]:
    start = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(hours=12)
    end = datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=days)
    with get_db() as conn:
        rows = conn.execute(
            "SELECT * FROM events WHERE start_utc >= ? AND start_utc <= ? ORDER BY start_utc",
            (start.isoformat()[:10], end.isoformat()),
        ).fetchall()
    return [dict(r) for r in rows]


async def poll_loop() -> None:
    while True:
        for fn in (sync_ics, sync_google, sync_icloud):
            try:
                n = await asyncio.to_thread(fn)
                if n:
                    log.info("%s: %d events", fn.__name__, n)
            except Exception:
                log.exception("%s failed", fn.__name__)
        await asyncio.sleep(config.calendar_poll_seconds)
