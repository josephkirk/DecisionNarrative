#ifndef STORYLET_ENGINE_H
#define STORYLET_ENGINE_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct DnRunHandle DnRunHandle;
typedef struct DnEngineHandle DnEngineHandle;
typedef struct DnPlaytestHandle DnPlaytestHandle;
typedef struct { uint8_t *data; size_t len; } DnBuffer;
DnRunHandle *dn_run_start(const char *content_json, const char *start_json, uint64_t seed, char **error_out);
char *dn_run_view(DnRunHandle *run, uint16_t participant);
char *dn_run_act(DnRunHandle *run, uint16_t participant, const char *action_json);
int32_t dn_run_rewind(DnRunHandle *run, uint32_t to_step);
void dn_run_free(DnRunHandle *run);
void dn_string_free(char *value);
uint32_t dn_engine_abi_version(void);
DnEngineHandle *dn_engine_create(void);
void dn_engine_destroy(DnEngineHandle *handle);
void dn_buffer_free(DnBuffer buffer);
bool dn_engine_import_snapshot(DnEngineHandle *handle, const uint8_t *data, size_t len);
DnBuffer dn_engine_export_snapshot(const DnEngineHandle *handle);
DnBuffer dn_engine_submit_command(DnEngineHandle *handle, const uint8_t *data, size_t len);
DnPlaytestHandle *dn_playtest_create(void);
void dn_playtest_destroy(DnPlaytestHandle *handle);
DnBuffer dn_playtest_start(DnPlaytestHandle *handle, const uint8_t *project_json, size_t project_len,
                           const uint8_t *scenario_id, size_t scenario_len);
DnBuffer dn_playtest_snapshot(const DnPlaytestHandle *handle);
DnBuffer dn_playtest_export_session(const DnPlaytestHandle *handle);
DnBuffer dn_playtest_restore_session(DnPlaytestHandle *handle, const uint8_t *session_json, size_t session_len);
DnBuffer dn_playtest_evaluate(const DnPlaytestHandle *handle, double target_tension_slope);
DnBuffer dn_playtest_submit_command(DnPlaytestHandle *handle, const uint8_t *command_json, size_t command_len);
DnBuffer dn_playtest_execute_choice(DnPlaytestHandle *handle, const uint8_t *choice_json, size_t choice_len);
#ifdef __cplusplus
}
#endif
#endif
