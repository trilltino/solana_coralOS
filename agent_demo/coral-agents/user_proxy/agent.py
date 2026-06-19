#!/usr/bin/env python3
"""
user-proxy — minimal idle Coral agent for integration testing.

Coral launches this container and injects CORAL_CONNECTION_URL.
The agent connects over MCP, then blocks indefinitely.
Its sole purpose is to be a named session participant that the
puppet API can impersonate to send messages to other agents.
"""
import asyncio
import os
import sys


def log(*a):
    print("[user-proxy]", *a, file=sys.stderr, flush=True)


async def main():
    from mcp import ClientSession
    from mcp.client.streamable_http import streamablehttp_client

    url = os.environ.get("CORAL_CONNECTION_URL")
    if not url:
        log("CORAL_CONNECTION_URL not set — must be launched by Coral.")
        sys.exit(1)

    log("connecting to Coral @", url)
    async with streamablehttp_client(url) as (read, write, _):
        async with ClientSession(read, write) as session:
            await session.initialize()
            tools = await session.list_tools()
            log("connected; tools:", [t.name for t in tools.tools])
            log("idle — puppet API is now active for this agent.")
            # Block forever; Coral's puppet API can inject messages on our behalf.
            await asyncio.Event().wait()


if __name__ == "__main__":
    asyncio.run(main())
