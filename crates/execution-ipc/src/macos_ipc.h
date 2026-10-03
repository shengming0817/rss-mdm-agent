// ref: macOS SDK Foundation/NSXPCConnection.h. One protocol declaration for product and probes.
#import <Foundation/Foundation.h>
@protocol RSSExecution
- (void)execute:(NSData *)request reply:(void (^)(NSData *))reply;
- (void)registerHelper:(NSXPCListenerEndpoint *)endpoint reply:(void (^)(BOOL))reply;
@end
