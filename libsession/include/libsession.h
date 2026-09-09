#ifndef LIBSESSION_H
#define LIBSESSION_H

#ifdef __cplusplus
extern "C" {
#endif

struct libsession_context;
struct libsession_session_id;

enum libsession_status {
    LIBSESSION_STATUS_OK = 0,
    LIBSESSION_STATUS_UNSUPPORTED = 1,
    LIBSESSION_STATUS_FAILED = 2,
};

enum libsession_capability {
    LIBSESSION_CAPABILITY_UNAVAILABLE = 0,
    LIBSESSION_CAPABILITY_AVAILABLE = 1,
    LIBSESSION_CAPABILITY_UNSUPPORTED = 2,
    LIBSESSION_CAPABILITY_FAILED = 3,
};

struct libsession_context *libsession_new(void);
void libsession_free(struct libsession_context *ctx);

struct libsession_session_id *libsession_get_session_id(struct libsession_context *ctx);
const char *libsession_session_id_str(const struct libsession_session_id *id);
void libsession_session_id_free(struct libsession_session_id *id);

enum libsession_status libsession_power_off(struct libsession_context *ctx);
enum libsession_status libsession_reboot(struct libsession_context *ctx);
enum libsession_status libsession_suspend(struct libsession_context *ctx);
enum libsession_status libsession_hibernate(struct libsession_context *ctx);
enum libsession_status libsession_hybrid_sleep(struct libsession_context *ctx);

enum libsession_capability libsession_can_power_off(struct libsession_context *ctx);
enum libsession_capability libsession_can_reboot(struct libsession_context *ctx);
enum libsession_capability libsession_can_suspend(struct libsession_context *ctx);
enum libsession_capability libsession_can_hibernate(struct libsession_context *ctx);
enum libsession_capability libsession_can_hybrid_sleep(struct libsession_context *ctx);

#ifdef __cplusplus
}
#endif

#endif
