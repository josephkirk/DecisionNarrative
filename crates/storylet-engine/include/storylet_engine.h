#ifndef STORYLET_ENGINE_H
#define STORYLET_ENGINE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct DnRunHandle DnRunHandle;
uint32_t dn_engine_abi_version(void);
DnRunHandle *dn_run_start(const char *content_json,const char *start_json,uint64_t seed,char **error_out);
char *dn_run_propose(DnRunHandle *run,const char *proposal_and_authority_json);
char *dn_run_observe(const DnRunHandle *run,uint16_t actor);
char *dn_run_explain(const DnRunHandle *run,uint32_t event_id);
char *dn_run_architect_view(const DnRunHandle *run,uint16_t actor);
void dn_run_free(DnRunHandle *run);
void dn_string_free(char *value);
#ifdef __cplusplus
}
#endif
#endif

