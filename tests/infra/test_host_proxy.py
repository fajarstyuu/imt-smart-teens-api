"""Host routing must not turn the private API name into a second public ingress."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class HostProxyContract(unittest.TestCase):
    def template(self, name):
        path = ROOT / "deploy/host-nginx" / name
        self.assertTrue(path.is_file(), "host vhost template missing")
        return path.read_text()

    def test_renewal_hook_is_scoped_and_validates_before_reload(self):
        p = ROOT / "deploy/host-nginx/renewal-hook.sh"
        self.assertTrue(p.is_file(), "webroot renewal must reload the correct Nginx certificate")
        text = p.read_text()
        self.assertIn("RENEWED_LINEAGE", text)
        self.assertIn("/etc/letsencrypt/live/imtsmartteens.my.id", text)
        self.assertLess(text.index("nginx -t"), text.index("systemctl reload nginx"))

    def test_frontend_tls_and_header_boundary(self):
        text = self.template("imtsmartteens.conf.template")
        for expected in ["server_name imtsmartteens.my.id", "127.0.0.1:5180", "proxy_set_header X-Real-IP $remote_addr", "proxy_set_header X-Forwarded-For $remote_addr", "proxy_set_header Forwarded", "listen [::]:443 ssl", "/etc/letsencrypt/live/imtsmartteens.my.id/fullchain.pem"]:
            self.assertIn(expected, text)
        self.assertNotIn("proxy_set_header Origin", text)
        self.assertNotIn("rizkyfs", text)

    def test_api_is_denied_except_acme_and_redirects_to_its_own_https_name(self):
        text = self.template("api-imtsmartteens.conf.template")
        self.assertIn("server_name api.imtsmartteens.my.id", text)
        self.assertIn("location / { return 403; }", text)
        self.assertIn("location ^~ /.well-known/acme-challenge/", text)
        self.assertIn("return 301 https://api.imtsmartteens.my.id$request_uri", text)
        self.assertNotIn("proxy_pass", text)
        self.assertIn("listen [::]:443 ssl", text)


if __name__ == "__main__":
    unittest.main()
