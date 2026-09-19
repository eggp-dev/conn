#!/bin/sh
# Create everything direct.mjs expects: a loopback-only SSH server with a small fake service,
# an `ssh staging` alias for the film shell, a synthetic password and the Conn profile.
# Nothing here touches your own SSH configuration, keys or Conn settings.
#   setup-fixture.sh [FILM_DIR]      default /tmp/conn-film (keep it short: Unix socket paths are limited)
#   setup-fixture.sh --reset [FILM_DIR]   put the server and Conn state back to the opening situation
set -eu
reset=no; [ "${1:-}" = "--reset" ] && { reset=yes; shift; }
F="${1:-/tmp/conn-film}"; NAME=conn-film-staging; IMAGE=conn-film-staging:local; PORT=22322
here="$(cd "$(dirname "$0")" && pwd)"

if [ "$reset" = yes ]; then
  docker exec "$NAME" sh -c 'pkill -u deploy -f http.server 2>/dev/null; cd /home/deploy/api && rm -f .api.pid && cp /opt/film/api.log api.log && touch cache/index.db cache/index.lock && chown -R deploy:deploy /home/deploy/api && rm -f /home/deploy/.bash_history'
  rm -rf "$F/state/chrome" "$F/state/audit.jsonl" "$F/state/conn.sock" "$F/state/connection.json"
  echo "reset: server stopped with a stale lock, Conn state cleared"; exit 0
fi

mkdir -p "$F/remote" "$F/home/.ssh" "$F/state" "$F/take" "$F/bin"; chmod 700 "$F" "$F/home/.ssh"
[ -f "$F/remote/host_key" ] || ssh-keygen -q -t ed25519 -N '' -f "$F/remote/host_key"
[ -f "$F/remote/password" ] || { printf 'SYNTHETIC_FILM_%s\n' "$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')" > "$F/remote/password"; chmod 600 "$F/remote/password"; }
chmod 644 "$F/remote/host_key.pub"

cat > "$F/remote/sshd_config" <<'CFG'
Port 22
ListenAddress 0.0.0.0
HostKey /etc/ssh/film_host_key
PasswordAuthentication yes
PubkeyAuthentication no
PermitRootLogin no
AllowUsers deploy
PrintMotd no
PrintLastLog no
LogLevel ERROR
AllowTcpForwarding no
X11Forwarding no
CFG

cat > "$F/remote/api.log" <<'LOG'
2026-09-19T04:11:52Z INFO  api listening on :8080
2026-09-19T09:40:03Z WARN  worker 3 restarted after timeout
2026-09-19T11:58:41Z ERROR upstream returned 502 for /v1/orders
2026-09-19T11:58:44Z FATAL cache/index.lock is held by pid 4121 (stale); refusing to start
2026-09-19T11:58:44Z INFO  api exited with status 1
LOG

cat > "$F/remote/api.sh" <<'API'
#!/bin/bash
# Disposable demo service for the Conn film. Not a real deployment tool.
cd "$(dirname "$0")"
pidfile=.api.pid
running() { [ -f "$pidfile" ] && kill -0 "$(cat "$pidfile")" 2>/dev/null; }
case "${1:-}" in
  status) if running; then echo "api: running (pid $(cat $pidfile)) on :8080"; else echo "api: stopped (last exit 1)"; fi ;;
  start)
    if running; then echo "api: already running"; exit 0; fi
    if [ -e cache/index.lock ]; then echo "api: refusing to start: cache/index.lock is held by pid 4121 (stale)"; exit 1; fi
    nohup python3 -c "
import http.server, json
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = json.dumps({'status': 'ok', 'cache': 'warm'}).encode()
        self.send_response(200); self.send_header('Content-Type', 'application/json'); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.HTTPServer(('127.0.0.1', 8080), H).serve_forever()" > /dev/null 2>&1 &
    echo $! > "$pidfile"; sleep 0.4; echo "api: started (pid $(cat $pidfile)) on :8080" ;;
  unlock) rm -f cache/index.lock; echo "cache: stale lock cleared, 1,204 entries kept" ;;
  health) python3 -c "
import urllib.request, sys
try: print(urllib.request.urlopen('http://127.0.0.1:8080/health', timeout=2).read().decode())
except Exception: print('health: connection refused'); sys.exit(1)" ;;
  *) echo "usage: ./api.sh status|start|unlock|health" ;;
esac
API
chmod +x "$F/remote/api.sh"

cat > "$F/remote/entry.sh" <<'ENTRY'
#!/bin/sh
set -e
install -m 600 /opt/film/host_key /etc/ssh/film_host_key
useradd -m -s /bin/bash deploy
printf 'deploy:%s\n' "$(cat /opt/film/password)" | chpasswd
mkdir -p /home/deploy/api/cache /run/sshd
cp /opt/film/api.sh /opt/film/api.log /home/deploy/api/
touch /home/deploy/api/cache/index.db /home/deploy/api/cache/index.lock
printf 'PS1="\\[\\e[38;5;114m\\]deploy@staging\\[\\e[0m\\]:\\[\\e[38;5;111m\\]\\w\\[\\e[0m\\]\\$ "\ncd ~/api\n' > /home/deploy/.bashrc
printf '[ -f ~/.bashrc ] && . ~/.bashrc\n' > /home/deploy/.bash_profile
chown -R deploy:deploy /home/deploy
exec /usr/sbin/sshd -D -e -f /opt/film/sshd_config
ENTRY
chmod +x "$F/remote/entry.sh"

# The film shell types `ssh staging`. OpenSSH reads its configuration from the account's real home,
# not $HOME, so a wrapper on the film shell's PATH points it at this fixture's own configuration.
printf '[127.0.0.1]:%s %s\n' "$PORT" "$(cut -d' ' -f1,2 "$F/remote/host_key.pub")" > "$F/home/.ssh/known_hosts"
cat > "$F/home/.ssh/config" <<CFG
Host staging
  HostName 127.0.0.1
  Port $PORT
  User deploy
  UserKnownHostsFile $F/home/.ssh/known_hosts
  StrictHostKeyChecking yes
  PreferredAuthentications password
  PubkeyAuthentication no
  IdentityAgent none
  LogLevel ERROR
CFG
chmod 600 "$F/home/.ssh/config"
printf '#!/bin/sh\nexec /usr/bin/ssh -F %s/home/.ssh/config "$@"\n' "$F" > "$F/bin/ssh"; chmod 755 "$F/bin/ssh"

# A local Bash profile keeps Conn's shell integration active, as it is for a user.
python3 - "$F" <<'PY'
import json, sys
F = sys.argv[1]
ps1 = r"\[\e[38;5;183m\]you@laptop\[\e[0m\]:\[\e[38;5;111m\]\w\[\e[0m\]\$ "
profile = {"id": "laptop", "name": "bash", "enabled": True, "backend": "local", "shell": "posix", "program": "/bin/bash", "args": ["--noprofile", "--norc"],
           "cwd": f"{F}/home", "env": {"HOME": f"{F}/home", "PS1": ps1, "TERM": "xterm-256color", "PATH": f"{F}/bin:/usr/local/bin:/usr/bin:/bin"}, "target": None, "port": None}
json.dump({"version": 1, "revision": 0, "defaultProfile": "laptop", "profiles": [profile]}, open(f"{F}/state/profiles.json", "w"))
PY

docker image inspect "$IMAGE" > /dev/null 2>&1 || docker build -q -t "$IMAGE" -f "$here/Dockerfile.staging" "$here" > /dev/null
docker rm -f "$NAME" > /dev/null 2>&1 || true
docker run -d --name "$NAME" --hostname staging -p "127.0.0.1:$PORT:22" --mount "type=bind,src=$F/remote,dst=/opt/film,readonly" "$IMAGE" /opt/film/entry.sh > /dev/null
echo "fixture ready in $F; server $NAME on 127.0.0.1:$PORT. Remove it with: docker rm -f $NAME"
