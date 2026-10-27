// The production publisher runs against a controlled NSXPC transport. No service,
// authorization, installation or login state is changed by this module test.
#import "../src/macos_service.m"
#import <objc/runtime.h>
#include <assert.h>
intptr_t rss_execution_call(void *c,uint32_t p,uint32_t u,uint32_t s,const uint8_t *b,size_t n,uint8_t *o,size_t z){return -1;}
int rss_execution_stopping(void){return 0;}
int rss_execution_allow_helper(void *c,uint32_t p,uint32_t u,uint32_t s){return 0;}
int rss_execution_helper_active(void){return 1;}
static NSMutableArray<NSXPCConnection *> *connections;
static NSMutableArray *replies;
@interface RSSPublicationRemote:NSObject<RSSExecution>
@end
@implementation RSSPublicationRemote
- (void)registerHelper:(NSXPCListenerEndpoint *)endpoint reply:(void (^)(BOOL))reply {
    [replies addObject:[reply copy]];
}
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply { assert(NO); }
@end
static void resume_connection(id self,SEL selector) { [connections addObject:self]; }
static id remote_connection(id self,SEL selector,void (^error)(NSError *)) { return [RSSPublicationRemote new]; }
static void invalidate_connection(id self,SEL selector) {
    NSXPCConnection *connection=self;
    if(connection.invalidationHandler)connection.invalidationHandler();
}
int main(void){@autoreleasepool{
    connections=[NSMutableArray new];replies=[NSMutableArray new];
    method_setImplementation(class_getInstanceMethod(NSXPCConnection.class,@selector(resume)),(IMP)resume_connection);
    method_setImplementation(class_getInstanceMethod(NSXPCConnection.class,@selector(remoteObjectProxyWithErrorHandler:)),(IMP)remote_connection);
    method_setImplementation(class_getInstanceMethod(NSXPCConnection.class,@selector(invalidate)),(IMP)invalidate_connection);
    NSXPCListener *listener=[NSXPCListener anonymousListener];
    RSSExecutionListener *delegate=[RSSExecutionListener new];listener.delegate=delegate;
    for(int i=0;i<12;i++)rss_publish_helper(listener,@"anchor apple");
    assert(connections.count==1 && "slow registration must not enqueue timer duplicates");
    void (^ack)(BOOL)=replies[0];ack(YES);
    rss_publish_helper(listener,@"anchor apple");
    assert(connections.count==2 && "acknowledgement must release publication admission");
    // A stale completion cannot release the next in-flight registration.
    ack(YES);rss_publish_helper(listener,@"anchor apple");
    assert(connections.count==2);
    [connections[1] invalidate];rss_publish_helper(listener,@"anchor apple");
    assert(connections.count==3 && "service loss must allow the existing timer to republish");
    (void)delegate;
}}
