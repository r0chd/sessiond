#ifndef LIBSESSION_H
#define LIBSESSION_H

#include <sys/types.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

struct libsession_context;

enum libsession_status {
    LIBSESSION_STATUS_OK = 0,
    LIBSESSION_STATUS_UNSUPPORTED = 1,
    LIBSESSION_STATUS_FAILED = 2,
};

struct libsession_context *libsession_new(void);
void libsession_free(struct libsession_context *ctx);

enum libsession_user_state {
    LIBSESSION_USER_STATE_OFFLINE = 0,
    LIBSESSION_USER_STATE_LINGERING = 1,
    LIBSESSION_USER_STATE_ONLINE = 2,
    LIBSESSION_USER_STATE_ACTIVE = 3,
    LIBSESSION_USER_STATE_CLOSING = 4,
};

enum libsession_session_state {
    LIBSESSION_STATE_ONLINE = 0,
    LIBSESSION_STATE_ACTIVE = 1,
    LIBSESSION_STATE_CLOSING = 2,
    LIBSESSION_STATE_OPENING = 3,
    LIBSESSION_STATE_UNKNOWN = 4,
};

enum libsession_status libsession_get_session(struct libsession_context *ctx, char **ret_session);
enum libsession_status libsession_pid_get_session(struct libsession_context *ctx, pid_t pid, char **ret_session);
void libsession_session_id_free(char *id);

enum libsession_status libsession_pid_get_owner_uid(struct libsession_context *ctx, pid_t pid, uid_t *ret_uid);
enum libsession_status libsession_uid_get_display(struct libsession_context *ctx, uid_t uid, char **ret_session);
enum libsession_status libsession_uid_get_state(struct libsession_context *ctx, uid_t uid, enum libsession_user_state *ret_state);
enum libsession_status libsession_session_get_uid(struct libsession_context *ctx, const char *session, uid_t *ret_uid);
enum libsession_status libsession_session_get_state(struct libsession_context *ctx, const char *session, enum libsession_session_state *ret_state);
enum libsession_status libsession_session_is_active(struct libsession_context *ctx, const char *session, bool *ret_active);
enum libsession_status libsession_session_is_local(struct libsession_context *ctx, const char *session, bool *ret_local);

#ifdef __cplusplus
}
#endif

#endif
