#include "../include/storylet_engine.h"
#include <stdio.h>
int main(void) {
    DnEngineHandle *engine = dn_engine_create();
    if (!engine || dn_engine_abi_version() != 1) return 1;
    DnBuffer snapshot = dn_engine_export_snapshot(engine);
    if (!snapshot.data || snapshot.len == 0) { dn_engine_destroy(engine); return 2; }
    printf("Storylet engine ABI %u; initial world snapshot: %.*s\n",
           dn_engine_abi_version(), (int)snapshot.len, (const char *)snapshot.data);
    dn_buffer_free(snapshot);
    dn_engine_destroy(engine);
    return 0;
}
