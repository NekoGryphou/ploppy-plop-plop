import asyncio
import argparse

from decky_power.client import HostClient
from decky_power.models import Device
from decky_power.protobuf import PROTOCOL_VERSION


async def main(code: str, port: int) -> None:
    device = Device(id="integration", name="Integration", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF", port=port)
    client = HostClient()
    credential, pairing = await client.pair(device, code)
    device.host_id = pairing.host_id
    status = await client.status(device, credential)
    assert status.protocol_version == PROTOCOL_VERSION and status.hostname
    await client.shutdown(device, credential)
    print("cross-language pairing, status, authenticated mock shutdown: ok")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--code", required=True)
    parser.add_argument("--port", required=True, type=int)
    arguments = parser.parse_args()
    asyncio.run(main(arguments.code, arguments.port))
