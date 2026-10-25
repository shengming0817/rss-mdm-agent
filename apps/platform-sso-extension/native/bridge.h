// Local C ABI only. Remote JSON declarations belong to rss-mdm-agent-wire.
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
typedef void RSSPSSOHandle;
typedef int32_t (*RSSPSSOSigner)(void *, const uint8_t *, size_t, uint8_t *);
RSSPSSOHandle *rss_psso_begin(uint32_t, uint32_t, uint64_t, const uint8_t *, const uint8_t *, int32_t *);
int32_t rss_psso_register(RSSPSSOHandle *, RSSPSSOSigner, void *);
const char *rss_psso_field(const RSSPSSOHandle *, uint32_t);
void rss_psso_free(RSSPSSOHandle *);
char *rss_psso_browser(uint32_t, const uint8_t *, const char *, const char *, const char *, const char *, const char *, RSSPSSOSigner, void *);
void rss_psso_string_free(char *);
int32_t rss_psso_observe(const char *, const char *, bool, bool);
int32_t rss_psso_unregister(const char *);
enum { RSSClient, RSSIssuer, RSSAudience, RSSNonce, RSSToken, RSSJwks, RSSLoginName, RSSMetadata, RSSLoginType, RSSAssertionType };
