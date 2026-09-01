#ifndef LIBSESSION_H
#define LIBSESSION_H

#ifdef __cplusplus
extern "C" {
#endif

struct libsession_context;
struct libsession_session_id;

struct libsession_context *libsession_new(void);
void libsession_free(struct libsession_context *ctx);

struct libsession_session_id *libsession_get_session_id(struct libsession_context *ctx);
const char *libsession_session_id_str(const struct libsession_session_id *id);
void libsession_session_id_free(struct libsession_session_id *id);

#ifdef __cplusplus
}
#endif

#endif
