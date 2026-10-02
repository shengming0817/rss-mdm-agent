// ref: Foundation/NSXPCConnection.h (macOS SDK). Transport facts are not product authority.
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <unistd.h>
extern intptr_t rss_execution_call(void *, uint32_t, uint32_t, uint32_t, const uint8_t *, size_t, uint8_t *, size_t);
extern int rss_execution_stopping(void);
extern int rss_execution_allow_helper(void *, uint32_t, uint32_t, uint32_t);
extern int rss_execution_helper_active(void);
static const NSUInteger RSS_FRAME_LIMIT = 8 * 1024 * 1024;
@protocol RSSExecution
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply;
- (void)registerHelper:(NSXPCListenerEndpoint *)endpoint reply:(void (^)(BOOL))reply;
@end
@interface RSSHelperRegistration:NSObject
@property(strong) NSXPCListenerEndpoint *endpoint;
@property uint32_t pid;
@property uint32_t uid;
@property uint32_t session;
@property uint64_t order;
@end
@implementation RSSHelperRegistration
@end
static NSMutableDictionary<NSString *,RSSHelperRegistration *> *rss_helpers(void) {
    static NSMutableDictionary *values;static dispatch_once_t once;
    dispatch_once(&once,^{values=[NSMutableDictionary new];});return values;
}
static NSString *rss_helper_key(uint32_t uid,uint32_t session){return [NSString stringWithFormat:@"%u/%u",uid,session];}
static uint64_t rss_registration_order;
@interface RSSExecutionPeer:NSObject<RSSExecution>
@property(weak) NSXPCConnection *connection;
@property BOOL consumed;
@end
@implementation RSSExecutionPeer
- (void)registerHelper:(NSXPCListenerEndpoint *)endpoint reply:(void (^)(BOOL))reply {
    @synchronized(self) {
        NSXPCConnection *connection=self.connection;
        if(!connection||_consumed||geteuid()!=0||!endpoint){reply(NO);return;}
        _consumed=YES;
        uint32_t uid=connection.effectiveUserIdentifier,session=connection.auditSessionIdentifier,pid=connection.processIdentifier;
        if(!rss_execution_allow_helper((__bridge void *)connection,pid,uid,session)){reply(NO);return;}
        NSMutableDictionary *values=rss_helpers();
        @synchronized(values){
            NSString *key=rss_helper_key(uid,session);RSSHelperRegistration *old=values[key];
            if((old&&old.pid!=pid)||(!old&&values.count>=64)){reply(NO);return;}
            RSSHelperRegistration *entry=[RSSHelperRegistration new];
            entry.endpoint=endpoint;entry.pid=pid;entry.uid=uid;entry.session=session;entry.order=++rss_registration_order;
            values[key]=entry;reply(YES);
        }
    }
}
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply {
    @synchronized(self) {
        NSXPCConnection *connection=self.connection;
        if (!connection||_consumed||request.length>RSS_FRAME_LIMIT) { reply([NSData data]); return; }
        _consumed=YES;
        NSMutableData *output=[NSMutableData dataWithLength:RSS_FRAME_LIMIT];
        intptr_t n=rss_execution_call((__bridge void *)connection,connection.processIdentifier,
            connection.effectiveUserIdentifier,connection.auditSessionIdentifier,
            request.bytes,request.length,output.mutableBytes,output.length);
        reply(n<0?[NSData data]:[NSData dataWithBytes:output.bytes length:(NSUInteger)n]);
    }
}
@end
@interface RSSExecutionListener:NSObject<NSXPCListenerDelegate>
@property NSUInteger active;
@property(copy) NSString *requirement;
@end
@implementation RSSExecutionListener
- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)connection {
    (void)listener;
    @synchronized(self){if(_active>=8||rss_execution_stopping()){fprintf(stderr,"RSS_IPC_SLOT_LIMIT active=%lu\n",(unsigned long)_active);return NO;}_active++;}
    if(_requirement){[connection setCodeSigningRequirement:_requirement];}
    RSSExecutionPeer *peer=[RSSExecutionPeer new];peer.connection=connection;
    connection.exportedInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
    connection.exportedObject=peer;
    __block BOOL counted=YES;
    connection.invalidationHandler=^{@synchronized(self){if(counted){counted=NO;self.active--;}}};
    [connection resume];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC),dispatch_get_global_queue(QOS_CLASS_DEFAULT,0),^{[connection invalidate];});
    return YES;
}
@end
// NSXPCListenerEndpoint is a native capability; no endpoint name or login identity is decoded from JSON.
static void rss_publish_helper(NSXPCListener *listener, NSString *requirement) {
    if(!rss_execution_helper_active())return;
    NSXPCConnection *connection=[[NSXPCConnection alloc]initWithMachServiceName:@"com.rss-mdm.agent.execution" options:NSXPCConnectionPrivileged];
    [connection setCodeSigningRequirement:requirement];
    connection.remoteObjectInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
    [connection resume];
    id<RSSExecution> remote=[connection remoteObjectProxyWithErrorHandler:^(NSError *error){(void)error;[connection invalidate];}];
    [remote registerHelper:listener.endpoint reply:^(BOOL accepted){(void)accepted;[connection invalidate];}];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC),dispatch_get_global_queue(QOS_CLASS_DEFAULT,0),^{[connection invalidate];});
}
int rss_execution_listen(const char *requirement){
    @autoreleasepool{
        RSSExecutionListener *delegate __attribute__((objc_precise_lifetime))=[RSSExecutionListener new];
        if(requirement){delegate.requirement=[NSString stringWithUTF8String:requirement];}
        NSString *name=geteuid()==0?@"com.rss-mdm.agent.execution":@"com.rss-mdm.agent.execution.user";
        NSXPCListener *listener=[[NSXPCListener alloc]initWithMachServiceName:name];listener.delegate=delegate;[listener resume];
        NSTimer *registration=nil;
        if(geteuid()!=0){
            rss_publish_helper(listener,delegate.requirement);
            registration=[NSTimer scheduledTimerWithTimeInterval:2 repeats:YES block:^(NSTimer *timer){(void)timer;rss_publish_helper(listener,delegate.requirement);}];
        }
        NSTimer *timer=[NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *timer){if(rss_execution_stopping()){[timer invalidate];CFRunLoopStop(CFRunLoopGetMain());}}];
        (void)timer; CFRunLoopRun();
        [registration invalidate];[listener invalidate];return 0;
    }
}
// Read-only lab/client transport. Product identity verification is supplied by #2564.
int rss_execution_query(const uint8_t *request,size_t length,int system,uint8_t *output,size_t *capacity,const char *requirement,uint32_t expectedUid){
    if(length>RSS_FRAME_LIMIT)return -1;
    @autoreleasepool{
        NSXPCConnection *connection=[[NSXPCConnection alloc]initWithMachServiceName:system?@"com.rss-mdm.agent.execution":@"com.rss-mdm.agent.execution.user" options:system?NSXPCConnectionPrivileged:0];
        if(requirement){[connection setCodeSigningRequirement:[NSString stringWithUTF8String:requirement]];}
        connection.remoteObjectInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
        dispatch_semaphore_t done=dispatch_semaphore_create(0);NSObject *lock=[NSObject new];__block NSData *result=nil;__block BOOL finished=NO;
        void (^finish)(NSData *)=^(NSData *bytes){@synchronized(lock){if(!finished){finished=YES;result=bytes;dispatch_semaphore_signal(done);}}};
        connection.interruptionHandler=^{finish(nil);};connection.invalidationHandler=^{finish(nil);};[connection resume];
        id<RSSExecution> remote=[connection remoteObjectProxyWithErrorHandler:^(NSError *error){fprintf(stderr,"RSS_IPC_TRANSPORT_ERROR code=%ld\n",(long)error.code);finish(nil);}];
        [remote execute:[NSData dataWithBytes:request length:length] reply:^(NSData *bytes){finish(bytes);}];
        dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC));
        @synchronized(lock){finished=YES;BOOL peerOk=expectedUid==UINT32_MAX||connection.effectiveUserIdentifier==expectedUid;[connection invalidate];if(!result||!result.length||result.length>*capacity)return -1;if(!peerOk)return -2;memcpy(output,result.bytes,result.length);*capacity=result.length;}
        return 0;
    }
}

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

// ref: Apple os/log.h; error-level unified logging is retained by the system log store.
#include <os/log.h>
void rss_execution_log(const char *message){
    static os_log_t log;
    static dispatch_once_t once;
    dispatch_once(&once,^{log=os_log_create("com.rss-mdm.agent.execution","mechanism");});
    os_log_error(log,"%{public}s",message);
}

// ref: macOS SDK bsm/audit.h getaudit_addr, kernel audit-session identity.
#include <bsm/audit.h>
int64_t rss_execution_audit_session(void) {
    auditinfo_addr_t info = {0};
    if (getaudit_addr(&info, sizeof(info)) != 0 || info.ai_asid < 0) return -1;
    return info.ai_asid;
}

// ref: macOS SDK CoreGraphics/CGSession.h. Inspect the caller's own GUI session, not a UID guess.
#import <CoreGraphics/CGSession.h>
int rss_execution_gui_active(void) {
    CFDictionaryRef session=CGSessionCopyCurrentDictionary();
    if(!session)return 0;
    CFTypeRef user=CFDictionaryGetValue(session,kCGSessionUserIDKey);
    int64_t uid=-1;
    BOOL valid=user&&CFGetTypeID(user)==CFNumberGetTypeID()&&CFNumberGetValue((CFNumberRef)user,kCFNumberSInt64Type,&uid)
        &&uid==geteuid()&&uid!=0
        &&CFDictionaryGetValue(session,kCGSessionOnConsoleKey)==kCFBooleanTrue
        &&CFDictionaryGetValue(session,kCGSessionLoginDoneKey)==kCFBooleanTrue;
    CFRelease(session);return valid;
}
int64_t rss_execution_helper_session(uint32_t uid) {
    if(geteuid()!=0)return -1;
    NSMutableDictionary *values=rss_helpers();
    @synchronized(values){
        RSSHelperRegistration *latest=nil;
        for(RSSHelperRegistration *entry in values.allValues){if(entry.uid==uid&&(!latest||entry.order>latest.order))latest=entry;}
        return latest?(int64_t)latest.session:-1;
    }
}
int rss_execution_helper_query(const uint8_t *request,size_t length,uint32_t uid,uint32_t session,uint8_t *output,size_t *capacity,const char *requirement) {
    if(geteuid()!=0||length>RSS_FRAME_LIMIT||!requirement)return -1;
    @autoreleasepool {
        NSMutableDictionary *values=rss_helpers();NSString *key=rss_helper_key(uid,session);
        RSSHelperRegistration *entry;
        @synchronized(values){entry=values[key];}
        if(!entry)return -1;
        NSXPCConnection *connection=[[NSXPCConnection alloc]initWithListenerEndpoint:entry.endpoint];
        [connection setCodeSigningRequirement:[NSString stringWithUTF8String:requirement]];
        connection.remoteObjectInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
        dispatch_semaphore_t done=dispatch_semaphore_create(0);NSObject *lock=[NSObject new];__block NSData *result=nil;__block BOOL finished=NO;
        void (^finish)(NSData *)=^(NSData *bytes){@synchronized(lock){if(!finished){finished=YES;result=bytes;dispatch_semaphore_signal(done);}}};
        connection.interruptionHandler=^{finish(nil);};connection.invalidationHandler=^{finish(nil);};[connection resume];
        id<RSSExecution> remote=[connection remoteObjectProxyWithErrorHandler:^(NSError *error){(void)error;finish(nil);}];
        [remote execute:[NSData dataWithBytes:request length:length] reply:^(NSData *bytes){finish(bytes);}];
        dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC));
        @synchronized(lock){
            finished=YES;BOOL peerOk=connection.effectiveUserIdentifier==uid&&(uint32_t)connection.auditSessionIdentifier==session&&(uint32_t)connection.processIdentifier==entry.pid;
            [connection invalidate];
            if(!result||!peerOk||result.length>*capacity){
                @synchronized(values){if(values[key]==entry)[values removeObjectForKey:key];}
                return -1;
            }
            memcpy(output,result.bytes,result.length);*capacity=result.length;
        }
        return 0;
    }
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

// ref: macOS SDK mach/mach_time.h. Continuous time includes system sleep.
#include <mach/mach_time.h>
uint64_t rss_execution_continuous_millis(void) {
    mach_timebase_info_data_t scale;
    if(mach_timebase_info(&scale)!=KERN_SUCCESS||scale.denom==0)return UINT64_MAX;
    return (uint64_t)(((__uint128_t)mach_continuous_time()*scale.numer)/scale.denom/1000000);
}
