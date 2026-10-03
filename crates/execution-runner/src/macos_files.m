#include <stdint.h>
#include <unistd.h>
#include <sys/acl.h>
#include <sys/stat.h>
#include <errno.h>
// Accept only read/search grants. Any ACL that grants replacement or mutation is rejected.
int rss_execution_fd_acl_restrictive(int fd){
    acl_t acl=acl_get_fd_np(fd,ACL_TYPE_EXTENDED);
    if(!acl){struct stat st;return errno==ENOENT&&fstat(fd,&st)==0;}
    acl_entry_t entry;int selector=ACL_FIRST_ENTRY;int valid=1;
    for(;;){
        errno=0;int rc=acl_get_entry(acl,selector,&entry);
        if(rc==-1){if(errno!=EINVAL)valid=0;break;}
        acl_tag_t tag;
        if(acl_get_tag_type(entry,&tag)!=0){valid=0;break;}
        if(tag==ACL_EXTENDED_ALLOW){
            acl_permset_mask_t mask;
            if(acl_get_permset_mask_np(entry,&mask)!=0 || (mask & ~(ACL_READ_DATA|ACL_EXECUTE|ACL_READ_ATTRIBUTES|ACL_READ_EXTATTRIBUTES|ACL_READ_SECURITY))!=0){valid=0;break;}
        } else if(tag!=ACL_EXTENDED_DENY){valid=0;break;}
        selector=ACL_NEXT_ENTRY;
    }
    acl_free(acl);return valid;
}

// ref: macOS SDK sys/acl.h and membership.h. Grant read/search only on a root-owned opened object.
#include <membership.h>
int rss_execution_grant_read(int fd,uint32_t uid) {
    struct stat st;uuid_t user;
    if(geteuid()!=0||uid==0||fstat(fd,&st)!=0||st.st_uid!=0||mbr_uid_to_uuid(uid,user)!=0)return -1;
    acl_t acl=acl_init(1);if(!acl)return -1;
    acl_entry_t entry;int result=-1;
    acl_permset_mask_t mask=ACL_READ_DATA|ACL_READ_ATTRIBUTES|ACL_READ_EXTATTRIBUTES|ACL_READ_SECURITY;
    if(S_ISDIR(st.st_mode))mask|=ACL_EXECUTE;
    if(acl_create_entry(&acl,&entry)==0&&acl_set_tag_type(entry,ACL_EXTENDED_ALLOW)==0&&acl_set_qualifier(entry,user)==0
        &&acl_set_permset_mask_np(entry,mask)==0&&acl_set_fd_np(fd,acl,ACL_TYPE_EXTENDED)==0)result=0;
    acl_free(acl);return result;
}

