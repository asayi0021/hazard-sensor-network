"""
Listen to the Public channel of a MeshCore companion node over Bluetooth LE and
write every message whose body is a valid JSON object straight to InfluxDB 3,
using the same measurement, tags and field names as live_plot.py.

Setup:
    pip install meshcore influxdb3-python
    $env:INFLUXDB3_TOKEN = "your_token_here"      (PowerShell)

Usage:
    python meshcore_influx_logger.py
    python meshcore_influx_logger.py -a AA:BB:CC:DD:EE:FF --pin 123456
    python meshcore_influx_logger.py --node 6 --location terania

The node must already be paired in Windows (Settings > Bluetooth & devices).
"""

import argparse
import asyncio
import json
import logging
import os
import time

from influxdb_client_3 import InfluxDBClient3, Point
from meshcore import MeshCore, EventType

MEASUREMENT = "sensor"

# Optional: rename keys in the incoming JSON to your InfluxDB field names,
# e.g. {"temp": "temperature", "aq": "air quality", "soil": "soil moisture"}
KEY_MAP = {}

# Fields stored as integers (everything else numeric is stored as float).
# "air quality" was written as an integer by live_plot.py, so it stays one.
INT_FIELDS = {"air quality"}


def build_fields(data: dict) -> dict:
    """Map JSON keys to InfluxDB fields with consistent types."""
    fields = {}
    for key, value in data.items():
        name = KEY_MAP.get(key, key)
        if isinstance(value, bool):
            fields[name] = value
        elif isinstance(value, (int, float)):
            fields[name] = int(value) if name in INT_FIELDS else float(value)
        elif isinstance(value, str):
            fields[name] = value
        # nested objects, lists and nulls are skipped
    return fields


async def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("-a", "--address", help="BLE address (omit to scan)")
    ap.add_argument("--pin", help="BLE pairing PIN, if the node has one set")
    ap.add_argument("--channel", type=int, default=0, help="channel index (default 0)")
    ap.add_argument("--host", default="http://localhost:8181")
    ap.add_argument("--database", default="sensors")
    ap.add_argument("--node", default="6", help="value of the 'node' tag")
    ap.add_argument("--location", default="terania", help="value of the 'location' tag")
    ap.add_argument("--debug", action="store_true")
    args = ap.parse_args()

    if args.debug:
        logging.basicConfig(level=logging.INFO)
        logging.getLogger("meshcore").setLevel(logging.DEBUG)

    influx = InfluxDBClient3(
        host=args.host,
        token=os.environ["INFLUXDB3_TOKEN"],  # keep the token out of source code
        database=args.database,
    )

    kwargs = {"debug": args.debug, "auto_reconnect": True, "max_reconnect_attempts": 5}
    if args.pin:
        kwargs["pin"] = args.pin

    mc = await MeshCore.create_ble(args.address, **kwargs)
    if mc is None:
        print("Could not connect. Check the node is on, not connected to a phone, and in range.")
        influx.close()
        return

    try:
        # Only continue if this channel index is the Public channel
        result = await mc.commands.get_channel(args.channel)
        if result.type == EventType.ERROR:
            print(f"Could not read channel {args.channel}: {result.payload}")
            return
        name = result.payload.get("channel_name")
        if name != "Public":
            print(f"Channel {args.channel} is NOT Public (name: {name!r}). Not logging.")
            return
        print(f"Channel {args.channel} is Public")

        async def on_channel_msg(event):
            text = event.payload.get("text") or ""
            print(text)  # every message, unfiltered

            # Messages arrive as "SenderName: payload"; split off the sender
            starts = [i for i in (text.find("{"), text.find("[")) if i != -1]
            if not starts:
                return
            start = min(starts)
            sender = text[:start].rstrip(": ").strip()

            try:
                parsed = json.loads(text[start:])
            except (ValueError, TypeError):
                return
            if not isinstance(parsed, dict):
                print("JSON is not an object, skipping.")
                return

            fields = build_fields(parsed)
            if not fields:
                print("JSON had no writable fields, skipping.")
                return

            point = (
                Point(MEASUREMENT)
                .tag("node", args.node)
                .tag("location", args.location)
            )
            if sender:
                point = point.tag("sender", sender)
            for key, value in fields.items():
                point = point.field(key, value)
            point = point.time(time.time_ns() // 1_000_000, write_precision="ms")

            try:
                # The client is blocking, so run it off the event loop
                await asyncio.to_thread(influx.write, record=point, write_precision="ms")
                print(f"write success: {fields}")
            except Exception as e:
                print(f"InfluxDB write failed: {e}")

        mc.subscribe(
            EventType.CHANNEL_MSG_RECV,
            on_channel_msg,
            attribute_filters={"channel_idx": args.channel},
        )
        await mc.start_auto_message_fetching()

        print(f"Listening. Writing to '{args.database}' as '{MEASUREMENT}'. Ctrl+C to stop.")
        await asyncio.Event().wait()
    finally:
        try:
            await mc.stop_auto_message_fetching()
        except Exception:
            pass
        await mc.disconnect()
        influx.close()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
