#ifndef STORYLET_ENGINE_H
#define STORYLET_ENGINE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* JSON arguments must point to valid NUL-terminated strings for the duration of the call.
   error_out may be NULL or point to writable storage for a char pointer.
   Run handles must be NULL or live handles returned by dn_run_start; mutable calls require exclusive access.
   Returned strings must be released once with dn_string_free; handles must be released once with dn_run_free.
   Never pass a freed pointer or a pointer obtained from another allocator. */
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

