import asyncio
import sys

from decky_power.client import HostClient
from decky_power.models import Device


async def main(code: str) -> None:
    device = Device(id="integration", name="Integration", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF", port=47991)
    client = HostClient()
    credential, pairing = await client.pair(device, code)
    device.host_id = pairing.host_id
    status = await client.status(device, credential)
    assert status.protocol_version == 1 and status.hostname
    await client.shutdown(device, credential)
    print("cross-language pairing, status, authenticated mock shutdown: ok")


if __name__ == "__main__": asyncio.run(main(sys.argv[1]))
