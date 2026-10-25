#include <stddef.h>
#include <stdint.h>
typedef struct RSSPasskeyHandle RSSPasskeyHandle;
int rss_passkey_begin(const uint8_t *, size_t, RSSPasskeyHandle **);
int rss_passkey_unlock(RSSPasskeyHandle *, int, const uint8_t *, size_t);
int rss_passkey_accounts(RSSPasskeyHandle *, char **);
int rss_passkey_execute(RSSPasskeyHandle *, const uint8_t *, size_t, char **);
int rss_passkey_check(RSSPasskeyHandle *);
void rss_passkey_cancel(RSSPasskeyHandle *);
void rss_passkey_free(RSSPasskeyHandle *);
void rss_passkey_free_text(char *);
int rss_passkey_validate(const char *);
