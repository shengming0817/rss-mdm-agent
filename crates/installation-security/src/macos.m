
#include <stdint.h>
#include <sys/acl.h>
#include <sys/stat.h>
#include <errno.h>

// This fixed lab installation uses POSIX modes only; any extended ACL is
// rejected rather than interpreting a second authorization policy.
int rss_acl_empty(const char *path) {
    acl_t acl = acl_get_file(path, ACL_TYPE_EXTENDED);
    if (!acl) {
        // Darwin reports ENOENT when an existing object has no extended ACL.
        struct stat metadata;
        return errno == ENOENT && lstat(path, &metadata) == 0;
    }
    acl_entry_t entry;
    errno = 0;
    int result = acl_get_entry(acl, ACL_FIRST_ENTRY, &entry);
    int empty = result == -1 && errno == EINVAL;
    acl_free(acl);
    return empty;
}
