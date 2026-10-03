// Test executable only. ref: macOS SDK Foundation/NSXPCConnection.h.
#import "macos_ipc.h"
#include <unistd.h>
#include <stdio.h>
#include <time.h>
#import <objc/runtime.h>

static NSString *const RSSFakeName = @"com.rss-mdm.agent.security.fake";
static void emit(NSDictionary *value) {
    NSData *bytes=[NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    if(!bytes)exit(2);
    fwrite(bytes.bytes,1,bytes.length,stdout);fputc('\n',stdout);fflush(stdout);
}
static double elapsed(void) {
    struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);
    return (double)t.tv_sec+(double)t.tv_nsec/1e9;
}
@interface RSSFakePeer:NSObject<RSSExecution>
@end
@implementation RSSFakePeer
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply {
    (void)request;
    fprintf(stderr,"RSS_FAKE_EXECUTE\n");fflush(stderr);
    reply([@"{\"fake\":true}" dataUsingEncoding:NSUTF8StringEncoding]);
}
- (void)registerHelper:(NSXPCListenerEndpoint *)endpoint reply:(void (^)(BOOL))reply {
    (void)endpoint;reply(NO);
}
@end
@interface RSSFakeListener:NSObject<NSXPCListenerDelegate>
@end
@implementation RSSFakeListener
- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)connection {
    (void)listener;
    connection.exportedInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
    connection.exportedObject=[RSSFakePeer new];[connection resume];return YES;
}
@end
@interface RSSProbeConnection:NSObject
@property NSXPCConnection *connection;
@property double opened;
@property BOOL invalidated;
@property BOOL interrupted;
@end
@implementation RSSProbeConnection
@end

int rss_security_probe_main(const char *requirement,int fake) {
    @autoreleasepool {
        if(fake) {
            RSSFakeListener *delegate __attribute__((objc_precise_lifetime))=[RSSFakeListener new];
            NSXPCListener *listener=[[NSXPCListener alloc]initWithMachServiceName:RSSFakeName];
            listener.delegate=delegate;[listener resume];CFRunLoopRun();return 0;
        }
        NSMutableDictionary<NSString *,RSSProbeConnection *> *connections=[NSMutableDictionary new];
        NSMutableArray<NSXPCListener *> *helpers=[NSMutableArray new];
        char *line=NULL;size_t capacity=0;ssize_t length;
        while((length=getline(&line,&capacity,stdin))>=0) {
            @autoreleasepool {
                if(length>16*1024*1024){free(line);return 2;}
                NSDictionary *command=[NSJSONSerialization JSONObjectWithData:
                    [NSData dataWithBytes:line length:(NSUInteger)length] options:0 error:nil];
                if(![command isKindOfClass:[NSDictionary class]]){free(line);return 2;}
                NSString *op=command[@"kind"],*identity=command[@"connection"]?:@"main";
                if(![identity isKindOfClass:[NSString class]]||identity.length>64){free(line);return 2;}
                if([op isEqual:@"stop"])break;
                if([op isEqual:@"open"]) {
                    if(connections[identity]||connections.count>=8){emit(@{@"error":@"connection limit or duplicate"});continue;}
                    BOOL isFake=[command[@"target"] isEqual:@"fake"];
                    BOOL untrusted=[command[@"untrusted"] boolValue];
                    if(untrusted&&!isFake){emit(@{@"error":@"untrusted baseline is fake-only"});continue;}
                    RSSProbeConnection *entry=[RSSProbeConnection new];
                    NSXPCConnection *connection=[[NSXPCConnection alloc]initWithMachServiceName:
                        isFake?RSSFakeName:@"com.rss-mdm.agent.execution" options:NSXPCConnectionPrivileged];
                    if(!untrusted)[connection setCodeSigningRequirement:[NSString stringWithUTF8String:requirement]];
                    connection.remoteObjectInterface=[NSXPCInterface interfaceWithProtocol:@protocol(RSSExecution)];
                    entry.connection=connection;entry.opened=elapsed();
                    __weak RSSProbeConnection *weakEntry=entry;
                    connection.invalidationHandler=^{RSSProbeConnection *e=weakEntry;@synchronized(e){e.invalidated=YES;}};
                    connection.interruptionHandler=^{RSSProbeConnection *e=weakEntry;@synchronized(e){e.interrupted=YES;}};
                    [connection resume];connections[identity]=entry;
                    if([command[@"establish"] boolValue]) {
                        // A nil endpoint is rejected before setting consumed in the production owner.
                        // This existing method establishes NSXPC without consuming the business one-shot.
                        dispatch_semaphore_t ready=dispatch_semaphore_create(0);
                        NSObject *guard=[NSObject new];__block BOOL answered=NO,accepted=YES,finished=NO;
                        id<RSSExecution> remote=[connection remoteObjectProxyWithErrorHandler:^(NSError *error){
                            (void)error;@synchronized(guard){if(!finished){finished=YES;dispatch_semaphore_signal(ready);}}
                        }];
                        [remote registerHelper:nil reply:^(BOOL allowed){@synchronized(guard){
                            if(!finished){finished=YES;answered=YES;accepted=allowed;dispatch_semaphore_signal(ready);}
                        }}];
                        dispatch_semaphore_wait(ready,dispatch_time(DISPATCH_TIME_NOW,2*NSEC_PER_SEC));
                        @synchronized(guard) {
                            finished=YES;
                            if(!answered||accepted||connection.effectiveUserIdentifier!=0||connection.processIdentifier<=1) {
                                [connection invalidate];[connections removeObjectForKey:identity];
                                emit(@{@"error":@"remote connection establishment not proven"});continue;
                            }
                            entry.opened=elapsed();
                            emit(@{@"created":@YES,@"established":@YES,@"connection":identity,
                                @"monotonic":@(entry.opened),@"peerUid":@(connection.effectiveUserIdentifier),
                                @"peerPid":@(connection.processIdentifier),@"peerSession":@(connection.auditSessionIdentifier),
                                @"method":@"registerHelper:nil",@"accepted":@NO});
                        }
                    } else emit(@{@"created":@YES,@"established":@NO,@"connection":identity,@"monotonic":@(entry.opened)});
                    continue;
                }
                RSSProbeConnection *entry=connections[identity];
                if(!entry){emit(@{@"error":@"unknown connection"});continue;}
                if([op isEqual:@"close"]) {
                    [entry.connection invalidate];[connections removeObjectForKey:identity];
                    emit(@{@"closed":@YES});continue;
                }
                if(![op isEqual:@"send"]&&![op isEqual:@"registerHelper"]){emit(@{@"error":@"unknown operation"});continue;}
                NSData *request=[[NSData alloc]initWithBase64EncodedString:command[@"payload"]?:@"" options:0];
                if(!request){emit(@{@"error":@"invalid payload encoding"});continue;}
                if(command[@"length"]) {
                    NSUInteger requested=[command[@"length"] unsignedIntegerValue];
                    if(requested>8*1024*1024+1){emit(@{@"error":@"probe payload bound"});continue;}
                    NSMutableData *expanded=[NSMutableData dataWithLength:requested];
                    if(command[@"padding"])memset(expanded.mutableBytes,[command[@"padding"] unsignedCharValue],requested);
                    memcpy(expanded.mutableBytes,request.bytes,MIN(request.length,requested));request=expanded;
                }
                double started=elapsed();
                dispatch_semaphore_t done=dispatch_semaphore_create(0);
                NSObject *lock=[NSObject new];__block NSDictionary *result=nil;__block BOOL finished=NO;
                void (^finish)(NSDictionary *)=^(NSDictionary *value){@synchronized(lock){if(!finished){finished=YES;result=value;dispatch_semaphore_signal(done);}}};
                id<RSSExecution> remote=[entry.connection remoteObjectProxyWithErrorHandler:^(NSError *error){
                    finish(@{@"transport":@"error",@"domain":error.domain,@"code":@(error.code)});
                }];
                if([op isEqual:@"registerHelper"]) {
                    RSSFakeListener *delegate=[RSSFakeListener new];
                    NSXPCListener *listener=[NSXPCListener anonymousListener];listener.delegate=delegate;
                    // Retain both until the process exits; a stale endpoint is explicit.
                    objc_setAssociatedObject(listener,@selector(registerHelper:reply:),delegate,OBJC_ASSOCIATION_RETAIN_NONATOMIC);
                    [listener resume];[helpers addObject:listener];
                    if([command[@"stale"] boolValue])[listener invalidate];
                    [remote registerHelper:listener.endpoint reply:^(BOOL accepted){finish(@{@"transport":@"reply",@"accepted":@(accepted)});}];
                } else {
                    [remote execute:request reply:^(NSData *bytes){
                        finish(@{@"transport":@"reply",@"bytes":@([bytes length]),@"replyBase64":[bytes base64EncodedStringWithOptions:0]});
                    }];
                }
                long waited=dispatch_semaphore_wait(done,dispatch_time(DISPATCH_TIME_NOW,4*NSEC_PER_SEC));
                @synchronized(lock) {
                    finished=YES;
                    NSMutableDictionary *reply=[(result?:@{@"transport":@"timeout"}) mutableCopy];
                    reply[@"connection"]=identity;reply[@"ageMs"]=@((started-entry.opened)*1000);
                    reply[@"elapsedMs"]=@((elapsed()-started)*1000);reply[@"payloadLength"]=@(request.length);
                    @synchronized(entry){reply[@"invalidated"]=@(entry.invalidated);reply[@"interrupted"]=@(entry.interrupted);}
                    if(!waited&&[reply[@"transport"] isEqual:@"reply"]) {
                        reply[@"peerUid"]=@(entry.connection.effectiveUserIdentifier);
                        reply[@"peerSession"]=@(entry.connection.auditSessionIdentifier);
                        reply[@"peerPid"]=@(entry.connection.processIdentifier);
                    }
                    emit(reply);
                }
            }
        }
        for(RSSProbeConnection *entry in connections.allValues)[entry.connection invalidate];
        for(NSXPCListener *listener in helpers)[listener invalidate];
        free(line);return 0;
    }
}
