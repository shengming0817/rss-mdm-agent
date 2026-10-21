// Native power/path observations only; scheduling and admission remain Rust-owned.
// ref: Apple Network.framework nw_path_monitor; IOKit/ps/IOPowerSources.h.
#import <Foundation/Foundation.h>
#import <Network/Network.h>
#import <IOKit/ps/IOPowerSources.h>
#import <IOKit/ps/IOPSKeys.h>
#include <stdatomic.h>
@interface RSSTelemetryEnvironment : NSObject {
@public
    _Atomic unsigned path_flags;
}
@property(nonatomic, strong) nw_path_monitor_t monitor;
@end
@implementation RSSTelemetryEnvironment
@end
void *rss_telemetry_environment_new(void) {
    RSSTelemetryEnvironment *owner = [RSSTelemetryEnvironment new];
    owner.monitor = nw_path_monitor_create();
    __weak RSSTelemetryEnvironment *weak = owner;
    nw_path_monitor_set_queue(owner.monitor, dispatch_get_global_queue(QOS_CLASS_UTILITY, 0));
    nw_path_monitor_set_update_handler(owner.monitor, ^(nw_path_t path) {
        RSSTelemetryEnvironment *current = weak;
        if (!current) return;
        unsigned flags = nw_path_get_status(path) == nw_path_status_satisfied ? 1 : 0;
        if (!nw_path_is_expensive(path) && !nw_path_is_constrained(path)) flags |= 2;
        atomic_store(&current->path_flags, flags);
    });
    nw_path_monitor_start(owner.monitor);
    return (__bridge_retained void *)owner;
}
unsigned rss_telemetry_environment(void *handle) {
    @autoreleasepool {
        RSSTelemetryEnvironment *owner = (__bridge RSSTelemetryEnvironment *)handle;
        unsigned result = atomic_load(&owner->path_flags);
        if (NSProcessInfo.processInfo.lowPowerModeEnabled) result |= 4;
        CFTypeRef info = IOPSCopyPowerSourcesInfo();
        if (!info) return result;
        CFArrayRef sources = IOPSCopyPowerSourcesList(info);
        if (sources) {
            result |= 32;
            for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
                NSDictionary *source = (__bridge NSDictionary *)IOPSGetPowerSourceDescription(info, CFArrayGetValueAtIndex(sources, i));
                if (!source) { result &= ~32u; continue; }
                if ([source[@kIOPSPowerSourceStateKey] isEqualToString:@kIOPSBatteryPowerValue]) {
                    result |= 8;
                    NSNumber *level = source[@kIOPSCurrentCapacityKey];
                    NSNumber *maximum = source[@kIOPSMaxCapacityKey];
                    if (!level || maximum.doubleValue <= 0) result &= ~32u;
                    else if (level.doubleValue / maximum.doubleValue <= .10) result |= 16;
                }
            }
            CFRelease(sources);
        }
        CFRelease(info);
        return result;
    }
}
void rss_telemetry_environment_stop(void *handle) {
    RSSTelemetryEnvironment *owner = (__bridge_transfer RSSTelemetryEnvironment *)handle;
    nw_path_monitor_cancel(owner.monitor);
    owner.monitor = nil;
}
