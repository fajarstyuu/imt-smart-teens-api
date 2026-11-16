#!/bin/sh
# certonly/webroot has no installer to reload Nginx; scope this hook to IMT only.
set -eu
case "${RENEWED_LINEAGE:-}" in
    /etc/letsencrypt/live/imtsmartteens.my.id)
        /usr/sbin/nginx -t
        /bin/systemctl reload nginx
        ;;
esac
