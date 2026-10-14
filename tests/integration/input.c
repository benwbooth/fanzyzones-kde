// SPDX-License-Identifier: MIT
// Test driver: refuse to connect to anything except session.sh's private socket.
#include "fake-input-client.h"
#include <wayland-client.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static struct org_kde_kwin_fake_input *input;
static void global(void *data, struct wl_registry *registry, uint32_t name, const char *interface, uint32_t version) {
    if (!strcmp(interface, "org_kde_kwin_fake_input"))
        input = wl_registry_bind(registry, name, &org_kde_kwin_fake_input_interface, version < 6 ? version : 6);
}
static void removed(void *data, struct wl_registry *registry, uint32_t name) {}
static const struct wl_registry_listener listener = {global, removed};
int main(void) {
    const char *socket = getenv("WAYLAND_DISPLAY"), *runtime = getenv("XDG_RUNTIME_DIR");
    if (!socket || strncmp(socket, "fanzyzones-test-", strlen("fanzyzones-test-")) || !runtime || strncmp(runtime, "/tmp/fanzyzones-parity.", strlen("/tmp/fanzyzones-parity."))) {
        fprintf(stderr, "Only private FanzyZones integration sessions are allowed\n"); return 1;
    }
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 2;
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &listener, NULL);
    wl_display_roundtrip(display);
    if (!input) return 3;
    org_kde_kwin_fake_input_authenticate(input, "FanzyZones integration test", "Private compositor input verification");
    wl_display_roundtrip(display);
    char line[128], command[32];
    double x, y;
    while (fgets(line, sizeof line, stdin)) {
        if (sscanf(line, "%31s %lf %lf", command, &x, &y) < 2) continue;
        if (!strcmp(command, "move")) org_kde_kwin_fake_input_pointer_motion_absolute(input, wl_fixed_from_double(x), wl_fixed_from_double(y));
        else if (!strcmp(command, "button")) org_kde_kwin_fake_input_button(input, (uint32_t)x, (uint32_t)y);
        else if (!strcmp(command, "key")) org_kde_kwin_fake_input_keyboard_key(input, (uint32_t)x, (uint32_t)y);
        else if (!strcmp(command, "wait")) usleep((useconds_t)(x * 1000));
        wl_display_roundtrip(display);
        usleep(10000);
    }
    org_kde_kwin_fake_input_destroy(input);
    wl_display_roundtrip(display);
    wl_display_disconnect(display);
    return 0;
}
