// ref: Foundation/NSXPCConnection.h (macOS SDK). Transport facts are not product authority.
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <unistd.h>
extern intptr_t rss_execution_call(void *, uint32_t, uint32_t, uint32_t, const uint8_t *, size_t, uint8_t *, size_t);
extern int rss_execution_stopping(void);
@protocol RSSExecution
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply;
@end
@interface RSSExecutionPeer:NSObject<RSSExecution>
@property(weak) NSXPCConnection *connection;
@property BOOL consumed;
@end
@implementation RSSExecutionPeer
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply {
    @synchronized(self) {
        NSXPCConnection *connection=self.connection;
        if (!connection||_consumed||request.length>65536) { reply([NSData data]); return; }
        _consumed=YES;
        uint8_t output[65536];
        intptr_t n=rss_execution_call((__bridge void *)connection,connection.processIdentifier,
            connection.effectiveUserIdentifier,connection.auditSessionIdentifier,
            request.bytes,request.length,output,sizeof(output));
        reply(n<0?[NSData data]:[NSData dataWithBytes:output length:(NSUInteger)n]);
    }
}
@end
@interface RSSExecutionListener:NSObject<NSXPCListenerDelegate>
@property NSUInteger active;
@end
@implementation RSSExecutionListener
- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)connection {
    (void)listener;
    @synchronized(self){if(_active>=64||rss_execution_stopping())return NO;_active++;}
    RSSExecutionPeer *peer=[RSSExecutionPeer new];peer.connection=connection;
    connection.exportedInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
    connection.exportedObject=peer;
    connection.invalidationHandler=^{@synchronized(self){self.active--;}};
    [connection resume];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC),dispatch_get_global_queue(QOS_CLASS_DEFAULT,0),^{[connection invalidate];});
    return YES;
}
@end
int rss_execution_listen(void){
    @autoreleasepool{
        RSSExecutionListener *delegate __attribute__((objc_precise_lifetime))=[RSSExecutionListener new];
        NSString *name=geteuid()==0?@"com.rss-mdm.agent.execution":@"com.rss-mdm.agent.execution.user";
        NSXPCListener *listener=[[NSXPCListener alloc]initWithMachServiceName:name];listener.delegate=delegate;[listener resume];
        NSTimer *timer=[NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *timer){if(rss_execution_stopping()){[timer invalidate];CFRunLoopStop(CFRunLoopGetMain());}}];
        (void)timer; CFRunLoopRun();
        [listener invalidate];return 0;
    }
}
// Read-only lab/client transport. Product identity verification is supplied by #2564.
int rss_execution_query(const uint8_t *request,size_t length,int system,uint8_t *output,size_t *capacity){
    if(length>65536)return -1;
    @autoreleasepool{
        NSXPCConnection *connection=[[NSXPCConnection alloc]initWithMachServiceName:system?@"com.rss-mdm.agent.execution":@"com.rss-mdm.agent.execution.user" options:system?NSXPCConnectionPrivileged:0];
        connection.remoteObjectInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
        dispatch_semaphore_t done=dispatch_semaphore_create(0);NSObject *lock=[NSObject new];__block NSData *result=nil;__block BOOL finished=NO;
        void (^finish)(NSData *)=^(NSData *bytes){@synchronized(lock){if(!finished){finished=YES;result=bytes;dispatch_semaphore_signal(done);}}};
        connection.interruptionHandler=^{finish(nil);};connection.invalidationHandler=^{finish(nil);};[connection resume];
        id<RSSExecution> remote=[connection remoteObjectProxyWithErrorHandler:^(NSError *error){(void)error;finish(nil);}];
        [remote execute:[NSData dataWithBytes:request length:length] reply:^(NSData *bytes){finish(bytes);}];
        dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,5*NSEC_PER_SEC));
        @synchronized(lock){finished=YES;[connection invalidate];if(!result||result.length>*capacity)return -1;memcpy(output,result.bytes,result.length);*capacity=result.length;}
        return 0;
    }
}

#include <sys/acl.h>
#include <sys/stat.h>
#include <errno.h>
// Reject granting extended ACLs; deny-only ACLs cannot enlarge POSIX write access.
int rss_execution_fd_acl_restrictive(int fd){
    acl_t acl=acl_get_fd_np(fd,ACL_TYPE_EXTENDED);
    if(!acl){struct stat st;return errno==ENOENT&&fstat(fd,&st)==0;}
    acl_entry_t entry;int selector=ACL_FIRST_ENTRY;int valid=1;
    for(;;){
        errno=0;int rc=acl_get_entry(acl,selector,&entry);
        if(rc==-1){if(errno!=EINVAL)valid=0;break;}
        acl_tag_t tag;
        if(acl_get_tag_type(entry,&tag)!=0||tag!=ACL_EXTENDED_DENY){valid=0;break;}
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
