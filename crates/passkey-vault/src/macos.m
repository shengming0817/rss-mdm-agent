// ref: Apple Foundation/NSFileManager.h; Security/SecItem.h (SDK 26.4).
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <unistd.h>
#include <string.h>
static NSString *const group = @"group.com.rss.mdmagent.personal-passkeys";
int rss_vault_root(char *out, size_t capacity) {
    @autoreleasepool {
        if (geteuid() == 0) return 0;
        SecTaskRef task = SecTaskCreateFromSelf(NULL);
        if (!task) return 0;
        CFTypeRef entitlement = SecTaskCopyValueForEntitlement(task, CFSTR("com.apple.security.application-groups"), NULL);
        CFRelease(task);
        NSArray *groups = CFBridgingRelease(entitlement);
        if (![groups isKindOfClass:NSArray.class] || ![groups containsObject:group]) return 0;
        NSURL *url = [NSFileManager.defaultManager containerURLForSecurityApplicationGroupIdentifier:group];
        if (!url) return 0;
        const char *path = [[url URLByAppendingPathComponent:@"personal-passkeys" isDirectory:YES] fileSystemRepresentation];
        if (!path || strlen(path) + 1 > capacity) return 0;
        memcpy(out, path, strlen(path) + 1);
        return 1;
    }
}
static NSMutableDictionary *query(const char *reference) {
    return [@{(__bridge id)kSecClass:(__bridge id)kSecClassGenericPassword,
        (__bridge id)kSecAttrService:@"RSS Personal Passkeys",
        (__bridge id)kSecAttrAccount:[NSString stringWithUTF8String:reference],
        (__bridge id)kSecAttrAccessGroup:group,
        (__bridge id)kSecUseDataProtectionKeychain:@YES,
        (__bridge id)kSecAttrSynchronizable:@NO} mutableCopy];
}
int rss_vault_key(const char *reference, unsigned char *bytes, int create) {
    @autoreleasepool {
        if (geteuid() == 0) return -1;
        NSMutableDictionary *q = query(reference);
        if (create) {
            q[(__bridge id)kSecValueData] = [NSData dataWithBytes:bytes length:32];
            q[(__bridge id)kSecAttrAccessible] = (__bridge id)kSecAttrAccessibleWhenUnlockedThisDeviceOnly;
            OSStatus status = SecItemAdd((__bridge CFDictionaryRef)q, NULL);
            return status == errSecSuccess ? 1 : -1;
        }
        q[(__bridge id)kSecReturnData] = @YES;
        q[(__bridge id)kSecMatchLimit] = (__bridge id)kSecMatchLimitOne;
        CFTypeRef result = NULL;
        OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)q, &result);
        NSData *data = CFBridgingRelease(result);
        if (status == errSecItemNotFound) return 0;
        if (status != errSecSuccess || ![data isKindOfClass:NSData.class] || data.length != 32) return -1;
        memcpy(bytes, data.bytes, 32);
        return 1;
    }
}

void rss_vault_notify_lock(void) {
    [NSDistributedNotificationCenter.defaultCenter postNotificationName:@"com.rss.mdmagent.personal-passkeys.lock" object:nil userInfo:nil deliverImmediately:YES];
}
