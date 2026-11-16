#!/usr/bin/env python3
"""Read-only public deployment checks; optional bounded invalid-login rate probe."""
import argparse
import hashlib
import http.client
import json
from pathlib import Path
import re
import socket
import ssl
import urllib.request
import urllib.error
import subprocess

FE = "https://imtsmartteens.my.id"
API = "https://api.imtsmartteens.my.id"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exercise-rate-limits", action="store_true")
    args = parser.parse_args()
    def request(url, *, origin=None, body=None, cookie=None, token=None, spoof="198.51.100.1"):
        headers = {"X-Real-IP": spoof, "X-Forwarded-For": spoof, "Forwarded": "for=" + spoof}
        if origin: headers["Origin"] = origin
        if cookie: headers["Cookie"] = cookie
        if token: headers["X-CSRF-Token"] = token
        if body is not None: headers["Content-Type"] = "application/json"
        req = urllib.request.Request(url, data=body, headers=headers)
        try: r = urllib.request.urlopen(req, timeout=15)
        except urllib.error.HTTPError as e: r = e
        return r.status, r.headers, r.read()
    for host in ["imtsmartteens.my.id", "api.imtsmartteens.my.id"]:
        with socket.create_connection((host, 443), timeout=10) as sock:
            with ssl.create_default_context().wrap_socket(sock, server_hostname=host) as tls:
                san = {value for kind, value in tls.getpeercert()["subjectAltName"] if kind == "DNS"}
                assert {"imtsmartteens.my.id", "api.imtsmartteens.my.id"} <= san
                print(host, "verified TLS", tls.version(), "SANs match")
        c = http.client.HTTPConnection(host, timeout=10)
        c.request("GET", "/")
        r = c.getresponse()
        assert r.status == 301 and r.getheader("Location") == f"https://{host}/"
        c.close()
    status, _, html = request(FE + "/")
    assert status == 200
    frontend = Path(__file__).resolve().parents[2]
    # The frontend worktree is supplied through the private nonsecret Compose env,
    # but public bytes are checked against the deployed container, not a guessed checkout.
    container = "imt-production-frontend-1"
    expected = subprocess.check_output(["sudo", "docker", "exec", container, "cat", "/usr/share/nginx/html/index.html"])
    assert html == expected
    for asset in re.findall(rb'(?:src|href)="([^\"]+\.(?:js|css))"', html):
        path = asset.decode()
        status, headers, content = request(FE + "/" + path.lstrip("/"))
        assert status == 200 and headers.get("Content-Type", "") != "text/html"
        local = subprocess.check_output(["sudo", "docker", "exec", container, "cat", "/usr/share/nginx/html/" + path.lstrip("/")])
        assert content == local
    assert request(FE + "/admin/masuk")[0] == 200
    assert request(FE + "/api/v1/config")[0] == 200
    for path in ["/", "/api/v1/config", "/api/v1/admin/sessions", "/health/ready"]:
        assert request(API + path)[0] == 403, "Direct API hostname exposed"
    for host, expected_status in [("imtsmartteens.my.id", "200"), ("api.imtsmartteens.my.id", "403")]:
        result = subprocess.check_output(["curl", "--noproxy", "*", "--resolve", host + ":443:[::1]", "--silent", "--show-error", "--max-time", "10", "-o", "/dev/null", "-w", "%{http_code}", "https://" + host + "/"], text=True)
        assert result == expected_status, "IPv6 host routing differs"
    status, headers, payload = request(FE + "/api/v1/csrf", origin=FE)
    assert status == 200 and headers.get("Access-Control-Allow-Origin") == FE
    cookies = headers.get_all("Set-Cookie")
    assert cookies and all("Secure" in c and "HttpOnly" in c and "SameSite=Lax" in c for c in cookies)
    assert all("Domain=" not in c for c in cookies)
    cookie = "; ".join(c.split(";", 1)[0] for c in cookies)
    token = json.loads(payload)["data"]["token"]
    bad_login = json.dumps({"email": "public-probe-nonexistent@example.invalid", "password": "synthetic-no-account-password"}).encode()
    status, headers, _ = request(FE + "/api/v1/admin/sessions", origin="https://evil.invalid", body=bad_login, cookie=cookie, token=token)
    assert status == 403 and headers.get("Access-Control-Allow-Origin") is None
    status, _, payload = request(FE + "/api/v1/admin/schools", origin=FE, body=b"x" * 65537)
    assert status == 413 and json.loads(payload)["error"]["code"] == "BODY_TOO_LARGE"
    if args.exercise_rate_limits:
        statuses = [request(FE + "/api/v1/admin/sessions", origin=FE, body=bad_login, cookie=cookie, token=token, spoof=f"198.51.100.{n + 2}")[0] for n in range(10)]
        # The foreign-origin attempt also consumes the real peer's login bucket.
        assert all(s == 401 for s in statuses[:9]) and statuses[9] == 429, "Forged IP headers evaded the edge boundary"
        print("Public spoofed IP headers cannot evade the unchanged login budget")
    status, _, portfolio = request("https://rizkyfs.my.id/")
    assert status == 200 and hashlib.sha256(portfolio).hexdigest() == "dc620b9b02378429cb1e8dbb81d1d60a3467ddf0f37306d5af00c854d4dc96a4"
    print("PASS: host-originated public HTTPS/SANs, redirects, exact container index/assets, SPA/API, IPv6 loopback routing, private API403, secure host-only cookies, exact CORS/CSRF, structured body limit and unchanged portfolio")


if __name__ == "__main__":
    main()
