#!/bin/sh
set -e

# Clean up legacy .list to prevent duplicate warnings (as per Section 1.2 & 5 of apt guide)
rm -f /etc/apt/sources.list.d/notion-lite.list /etc/apt/sources.list.d/notion.list /etc/apt/sources.list.d/lotion-rs.list

# Update system icon cache and desktop database so icons appear immediately
if which gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if which update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi

exit 0
