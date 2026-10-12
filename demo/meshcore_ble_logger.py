"""
Log JSON messages from the Public channel of a MeshCore companion node over
Bluetooth LE, using meshcore_py.

On connecting, it checks whether the channel at the chosen index is the Public
channel and prints a single line saying so. If it is not Public, it exits.
If it is, it listens and, for each message on that channel:

  - CLI: print raw public channel message including node name prefix.
  - public_channel_log.jsonl : only messages whose full text is valid JSON
  - public_channel_raw.txt   : only messages whose full text is valid JSON

Usage:
    python meshcore_ble_logger.py                       # scan and connect to first MeshCore device found
    python meshcore_ble_logger.py -a AA:BB:CC:DD:EE:FF  # connect to a specific address
    python meshcore_ble_logger.py -a <addr> --pin 123456
    python meshcore_ble_logger.py --channel 0           # channel index to check/listen on (default 0)

On macOS the BLE "address" is a UUID that CoreBluetooth assigns, not a MAC.
"""

import argparse
import asyncio
import json
import logging
from datetime import datetime, timezone

from meshcore import MeshCore, EventType

JSONL_FILE = "public_channel_log.jsonl"
RAW_FILE = "public_channel_raw.txt"


def _json_default(obj):
    if isinstance(obj, (bytes, bytearray)):
        return obj.hex()
    return str(obj)


async def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("-a", "--address", help="BLE address (omit to scan)")
    ap.add_argument("--pin", help="BLE pairing PIN, if the node has one set")
    ap.add_argument("--channel", type=int, default=0,
                    help="channel index to check and listen on (default 0)")
    ap.add_argument("--debug", action="store_true")
    args = ap.parse_args()

    if args.debug:
        logging.basicConfig(level=logging.INFO)
        logging.getLogger("meshcore").setLevel(logging.DEBUG)

    kwargs = {"debug": args.debug, "auto_reconnect": True, "max_reconnect_attempts": 5}
    if args.pin:
        kwargs["pin"] = args.pin

    mc = await MeshCore.create_ble(args.address, **kwargs)
    if mc is None:
        print("Could not connect. Check the node is on, not connected to a phone, and in range.")
        return

    try:
        # --- Initial check: is the channel at this index the Public channel? ---
        result = await mc.commands.get_channel(args.channel)
        if result.type == EventType.ERROR:
            print(f"Could not read channel {args.channel}: {result.payload}")
            return

        name = result.payload.get("channel_name")
        if name != "Public":
            print(f"Channel {args.channel} is NOT Public (name: {name!r}). Not logging.")
            return
        print(f"Channel {args.channel} is Public")

        # --- Logging ---
        async def on_channel_msg(event):
            p = event.payload
            text = p.get("text") or ""
            ts = datetime.now(timezone.utc).isoformat()

            # CLI log: every message on the channel, unfiltered
            print(f"{text}")

            # Raw log: only messages that are entirely valid JSON (object or array) to .txt
            # filters out the node name prefix to the message.

            # Channel messages arrive as "SenderName: payload". Split off the sender,
            # then require the remainder to be entirely valid JSON.
            starts = [i for i in (text.find("{"), text.find("[")) if i != -1]
            if not starts:
                return
            start = min(starts)
            sender = text[:start].rstrip(": ").strip()
            body = text[start:]

            try:
                parsed = json.loads(body)
            except (ValueError, TypeError):
                return
            if not isinstance(parsed, (dict, list)):
                return

            print(f"JSON parsed with match, data logged.")

            with open(RAW_FILE, "a", encoding="utf-8") as f:
                f.write(f"{body}\n")

            # JSONL log: only messages that are entirely valid JSON (object or array)
            line = json.dumps({"ts": ts, **p, "json": parsed}, default=_json_default)
            with open(JSONL_FILE, "a", encoding="utf-8") as f:
                f.write(line + "\n")

        mc.subscribe(
            EventType.CHANNEL_MSG_RECV,
            on_channel_msg,
            attribute_filters={"channel_idx": args.channel},
        )
        await mc.start_auto_message_fetching()

        print(f"Listening. Logging to {JSONL_FILE} and {RAW_FILE}. Ctrl+C to stop.")
        await asyncio.Event().wait()
    finally:
        try:
            await mc.stop_auto_message_fetching()
        except Exception:
            pass
        await mc.disconnect()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
