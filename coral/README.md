# Running a real Coral Server (free, self-hosted)

Coral Server is open-source. You run it yourself via Docker — there is no free
public endpoint. This folder is the one-command setup the Helius agent will
connect to.

## Facts

- Image: `ghcr.io/coral-protocol/coral-server`
- Port: **5555** (so the local URL is `http://localhost:5555`)
- Auth: a key, simplest is `--auth.keys=dev` (dev only)
- It mounts the **Docker socket** because Coral launches each agent as its own
  Docker container (the "Docker runtime").

## Prereq

Docker Desktop must be **running** (whale icon steady in the tray). Verify:

```sh
docker version        # Server section must print, no pipe error
```

## Start it

```sh
cd coral
docker compose up
```

Then in another terminal:

```sh
curl http://localhost:5555/        # server should answer
```

> The exact health/route path and config schema are confirmed against the image
> on first run (the project README is explicitly "a work in progress"). If the
> `command:` args below are rejected by the image's entrypoint, switch to the
> `CONFIG_FILE_PATH` form shown in comments inside `docker-compose.yml`.

## Official reference command (equivalent, no compose)

```sh
docker run -p 5555:5555 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  ghcr.io/coral-protocol/coral-server --auth.keys=dev
```

Source: https://github.com/Coral-Protocol/coral-server  ·  https://docs.coralos.ai
