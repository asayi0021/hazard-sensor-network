"""Writes a random temperature reading to InfluxDB 3 every 2 seconds.

Install:  pip install influxdb3-python
"""
import os
import random
import time

from influxdb_client_3 import InfluxDBClient3, Point

client = InfluxDBClient3(
    host="http://localhost:8181",
    token=os.environ["INFLUXDB3_TOKEN"],  # keep the token out of source code
    database="sensors",
)

try:
    while True:
        point = (
            Point("sensor")
            .tag("node", "6")
            .tag("location", "terania")
            .field("temperature", random.uniform(18.0, 26.0))
            .field("humidity", random.uniform(40.0, 45.0))
            .field("pressure", random.uniform(1012.25, 1014.25))
            .field("air quality", random.randint(0,2))
            .field("soil moisture", random.uniform(25.0, 30.0))
            .field("wind speed", random.gauss(25.0,1.0))
            .field("rainfall", random.gauss(40,1))
            .field("battery", 87.0)
            .time(time.time_ns() // 1_000_000, write_precision="ms")  # current system time, in ms
        )

        client.write(record=point, write_precision="ms")
        print("write success")
        time.sleep(2)
except KeyboardInterrupt:
    pass
finally:
    client.close()
