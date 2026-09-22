// NSXPCConnection's peer requirement is enforced by the OS on each message.
// ref: Foundation/NSXPCConnection.h (macOS SDK 26.4), no private audit-token APIs.
#import <Foundation/Foundation.h>
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
extern void *rss_peer_new(uint32_t, uint32_t, int);
extern void rss_peer_free(void *);
extern intptr_t rss_peer_greeting(void *, uint8_t *, size_t);
extern intptr_t rss_peer_request(void *, const uint8_t *, size_t, uint8_t *, size_t);
extern intptr_t rss_query_request(const uint8_t *, size_t, uint8_t *, size_t);
extern intptr_t rss_query_reply(const uint8_t *, size_t, const uint8_t *, size_t, uint8_t *, size_t);

@protocol RSSStatus
- (void)greeting:(void (^)(NSData *))reply;
- (void)status:(NSData *)request reply:(void (^)(NSData *))reply;
@end
@interface RSSPeer : NSObject <RSSStatus>
@property void *context;
@property BOOL greeted;
@property(weak) NSXPCConnection *connection;
@end
@implementation RSSPeer
- (void)dealloc { rss_peer_free(_context); }
- (void)greeting:(void (^)(NSData *))reply {
    @synchronized(self) {
        if (_greeted) { reply([NSData data]); [_connection invalidate]; return; }
        _greeted = YES;
        uint8_t output[65536];
        intptr_t size = rss_peer_greeting(_context, output, sizeof(output));
        reply(size < 0 ? [NSData data] : [NSData dataWithBytes:output length:(NSUInteger)size]);
    }
}
- (void)status:(NSData *)request reply:(void (^)(NSData *))reply {
    @synchronized(self) {
        uint8_t output[65536];
        intptr_t size = _greeted && request.length <= 65536
            ? rss_peer_request(_context, request.bytes, request.length, output, sizeof(output)) : -1;
        reply(size < 0 ? [NSData data] : [NSData dataWithBytes:output length:(NSUInteger)size]);
        // The Rust context is consumed before reply; another request cannot run.
    }
}
@end
@interface RSSListener : NSObject <NSXPCListenerDelegate>
@property(copy) NSString *requirement;
@property NSUInteger active;
@end
@implementation RSSListener
- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)connection {
    (void)listener;
    @synchronized(self) { if (_active >= 64) return NO; _active++; }
    [connection setCodeSigningRequirement:_requirement];
    void *context = rss_peer_new(connection.effectiveUserIdentifier,
                               connection.auditSessionIdentifier, connection.processIdentifier);
    if (!context) { @synchronized(self) { _active--; } return NO; }
    RSSPeer *peer = [RSSPeer new]; peer.context = context; peer.connection = connection;
    connection.exportedInterface = [NSXPCInterface interfaceWithProtocol:@protocol(RSSStatus)];
    connection.exportedObject = peer;
    connection.invalidationHandler = ^{ @synchronized(self) { self.active--; } };
    [connection resume];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC),
                   dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{ [connection invalidate]; });
    return YES;
}
@end
int rss_service_run(const char *requirement) {
    @autoreleasepool {
        RSSListener *delegate __attribute__((objc_precise_lifetime)) = [RSSListener new];
        delegate.requirement = [NSString stringWithUTF8String:requirement];
        NSXPCListener *listener = [[NSXPCListener alloc] initWithMachServiceName:@"com.rss-mdm.agent.status"];
        listener.delegate = delegate;
        [listener resume];
        dispatch_main();
    }
    return 0;
}
int rss_service_query(const char *requirement, uint32_t uid, uint8_t *output, size_t *size) {
    @autoreleasepool {
        NSXPCConnection *connection = [[NSXPCConnection alloc]
            initWithMachServiceName:@"com.rss-mdm.agent.status" options:NSXPCConnectionPrivileged];
        [connection setCodeSigningRequirement:[NSString stringWithUTF8String:requirement]];
        connection.remoteObjectInterface = [NSXPCInterface interfaceWithProtocol:@protocol(RSSStatus)];
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        NSObject *lock = [NSObject new];
        __block NSData *result = nil;
        __block BOOL finished = NO;
        void (^finish)(NSData *) = ^(NSData *bytes) {
            @synchronized(lock) { if (!finished) { finished = YES; result = bytes; dispatch_semaphore_signal(done); } }
        };
        connection.interruptionHandler = ^{ finish(nil); };
        connection.invalidationHandler = ^{ finish(nil); };
        [connection resume];
        id<RSSStatus> remote = [connection remoteObjectProxyWithErrorHandler:^(NSError *error) {
            (void)error; finish(nil);
        }];
        [remote greeting:^(NSData *greeting) {
            if (connection.effectiveUserIdentifier != uid || greeting.length > 65536) { finish(nil); return; }
            uint8_t request[65536];
            intptr_t count = rss_query_request(greeting.bytes, greeting.length, request, sizeof(request));
            if (count < 0) { finish(nil); return; }
            [remote status:[NSData dataWithBytes:request length:(NSUInteger)count] reply:^(NSData *reply) {
                uint8_t value[65536];
                intptr_t n = rss_query_reply(greeting.bytes, greeting.length, reply.bytes, reply.length, value, sizeof(value));
                finish(n < 0 ? nil : [NSData dataWithBytes:value length:(NSUInteger)n]);
            }];
        }];
        dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC));
        @synchronized(lock) {
            finished = YES;
            [connection invalidate];
            if (!result || result.length > *size) return -1;
            memcpy(output, result.bytes, result.length); *size = result.length;
        }
        return 0;
    }
}
