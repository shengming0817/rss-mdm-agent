// ref: Apple OpenDirectory/ODQuery.h, Security/SecItem.h, AppKit/NSWorkspace.h (SDK 26.4).
#import <Foundation/Foundation.h>
#import <OpenDirectory/OpenDirectory.h>
#import <Security/Security.h>
#import <SystemConfiguration/SystemConfiguration.h>
#import <AppKit/AppKit.h>
typedef int (*Emitter)(void *, const uint8_t *, size_t);
static int json(NSDictionary *row, Emitter emit, void *state) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:row options:0 error:NULL];
    return data ? emit(state, data.bytes, data.length) : -1;
}
static NSString *one(ODRecord *r, NSString *key) {
    NSError *error = nil;
    NSArray *a = [r valuesForAttribute:key error:&error];
    return !error && a.count == 1 && [a[0] isKindOfClass:NSString.class] ? a[0] : nil;
}
static NSArray *query(ODNode *node, id types, NSString *attribute, ODMatchType match, id values, NSArray *attrs, NSUInteger limit, NSError **error) {
    ODQuery *q = [ODQuery queryWithNode:node forRecordTypes:types attribute:attribute matchType:match queryValues:values returnAttributes:attrs maximumResults:limit error:error];
    return q ? [q resultsAllowingPartial:NO error:error] : nil;
}
static NSString *guid(ODRecord *r) {
    NSString *value = one(r, kODAttributeTypeGUID);
    NSUUID *uuid = value ? [[NSUUID alloc] initWithUUIDString:value] : nil;
    return uuid ? uuid.UUIDString.lowercaseString : nil;
}
int rss_macos_console_account(const char *name) {
    if (!name) return 0;
    NSString *value = [NSString stringWithUTF8String:name];
    return value.length && ![value isEqual:@"loginwindow"] && ![value isEqual:@"_mbsetupuser"];
}
int rss_macos_accounts(uint32_t section, const uint8_t *selection, size_t length, size_t source_limit, Emitter emit, void *state) {
    @autoreleasepool {
        if (source_limit == 0 || source_limit > 4096 || length > 16384) return -1;
        NSArray *selected = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:selection length:length] options:0 error:NULL];
        if (![selected isKindOfClass:NSArray.class]) return -1;
        NSSet *approved = [NSSet setWithArray:selected];
        NSMutableArray *directoryIDs = [NSMutableArray array];
        for (NSString *identity in selected) [directoryIDs addObject:identity.uppercaseString];
        NSError *error = nil;
        ODNode *node = [ODNode nodeWithSession:ODSession.defaultSession type:kODNodeTypeLocalNodes error:&error];
        if (!node || error) return 2;
        NSArray *attrs = @[kODAttributeTypeGUID, kODAttributeTypeUniqueID, kODAttributeTypeRecordName];
        if (section == 0) {
            NSArray *records = query(node, kODRecordTypeUsers, selected.count ? kODAttributeTypeGUID : kODAttributeTypeRecordName,
                selected.count ? kODMatchEqualTo : kODMatchAny, selected.count ? directoryIDs : nil, attrs, source_limit + 1, &error);
            if (!records || error) return 2;
            if (records.count > source_limit) return 4;
            BOOL partial = NO;
            NSMutableSet *seen = [NSMutableSet set];
            for (ODRecord *r in records) {
                NSString *identity = guid(r), *uid = one(r, kODAttributeTypeUniqueID), *name = r.recordName;
                unsigned long long numeric = 0;
                NSScanner *scanner = uid ? [NSScanner scannerWithString:uid] : nil;
                if (!identity || !name || ![scanner scanUnsignedLongLong:&numeric] || !scanner.isAtEnd || numeric > UINT32_MAX) { partial = YES; continue; }
                if (selected.count && ![approved containsObject:identity]) return -1;
                [seen addObject:identity];
                if (json(@{@"id":identity, @"uid":@(numeric), @"name":name, @"account_type":@"local", @"enabled":@"unknown"}, emit, state)) return -1;
            }
            // No credential attributes are read to infer login eligibility.
            return partial || (selected.count && ![seen isEqualToSet:approved]) ? 1 : 0;
        }
        if (section == 1) {
            NSArray *groups = query(node, kODRecordTypeGroups, kODAttributeTypeRecordName, kODMatchEqualTo, @"admin",
                @[kODAttributeTypeGroupMembers, kODAttributeTypeNestedGroups, kODAttributeTypeGroupMembership], 2, &error);
            if (error || groups.count != 1) return 2;
            ODRecord *admin = groups[0];
            NSMutableDictionary *members = [NSMutableDictionary dictionary];
            for (NSString *attribute in @[kODAttributeTypeGroupMembers, kODAttributeTypeNestedGroups]) {
                NSArray *values = [admin valuesForAttribute:attribute error:&error];
                if (error) return 2;
                if (values.count > source_limit) return 4;
                for (NSString *value in values) {
                    NSUUID *uuid = [value isKindOfClass:NSString.class] ? [[NSUUID alloc] initWithUUIDString:value] : nil;
                    if (!uuid) return -1;
                    members[uuid.UUIDString.lowercaseString] = [attribute isEqual:kODAttributeTypeNestedGroups] ? @"group" : @"user";
                }
            }
            // Legacy direct name membership is resolved locally, without recursive groups.
            NSArray *names = [admin valuesForAttribute:kODAttributeTypeGroupMembership error:&error];
            if (error || names.count > source_limit) return error ? 2 : 4;
            BOOL partial = NO;
            for (NSString *name in names) {
                NSArray *r = query(node, kODRecordTypeUsers, kODAttributeTypeRecordName, kODMatchEqualTo, name, attrs, 2, &error);
                if (error) return 2;
                NSString *identity = r.count == 1 ? guid(r[0]) : nil;
                if (identity) members[identity] = @"user"; else partial = YES;
            }
            if (members.count > source_limit) return 4;
            for (NSString *identity in [[members allKeys] sortedArrayUsingSelector:@selector(compare:)]) {
                if (selected.count && ![approved containsObject:identity]) continue;
                NSString *type = members[identity];
                NSArray *r = query(node, [type isEqual:@"group"] ? kODRecordTypeGroups : kODRecordTypeUsers,
                    kODAttributeTypeGUID, kODMatchEqualTo, identity.uppercaseString, attrs, 2, &error);
                if (error) return 2;
                if (json(@{@"id":[@"admin:" stringByAppendingString:identity], @"member":identity, @"member_type":type, @"resolved":r.count == 1 ? @"resolved" : @"unresolved"}, emit, state)) return -1;
            }
            return partial ? 1 : 0;
        }
        if (section == 2) {
            uid_t uid = 0; gid_t gid = 0;
            NSString *name = CFBridgingRelease(SCDynamicStoreCopyConsoleUser(NULL, &uid, &gid));
            if (rss_macos_console_account(name.UTF8String)) {
                NSArray *r = query(node, kODRecordTypeUsers, kODAttributeTypeUniqueID, kODMatchEqualTo, [NSString stringWithFormat:@"%u",uid], attrs, 2, &error);
                NSString *identity = !error && r.count == 1 ? guid(r[0]) : nil;
                if (!identity) return 1;
                if ((!selected.count || [approved containsObject:identity]) && json(@{@"id":[@"console:" stringByAppendingString:identity], @"owner":identity, @"state":@"active"}, emit, state)) return -1;
            }
            // A console observation cannot prove the absence of other login sessions.
            return 1;
        }
        return -1;
    }
}
int rss_macos_certificates(const char *path, size_t maximum, Emitter emit, void *state) {
    @autoreleasepool {
        if (maximum == 0 || maximum > 4096) return -1;
        SecKeychainRef keychain = NULL;
        OSStatus status = SecKeychainSetUserInteractionAllowed(false);
        if (status != errSecSuccess) return 2;
        status = SecKeychainOpen(path, &keychain);
        if (status != errSecSuccess) return status == errSecAuthFailed || status == errSecInteractionNotAllowed ? 2 : 3;
        NSDictionary *request = @{(__bridge id)kSecClass:(__bridge id)kSecClassCertificate,
            (__bridge id)kSecMatchSearchList:@[(__bridge id)keychain],
            (__bridge id)kSecReturnRef:@YES, (__bridge id)kSecMatchLimit:@(maximum + 1),
            (__bridge id)kSecUseAuthenticationUI:(__bridge id)kSecUseAuthenticationUIFail};
        CFTypeRef result = NULL;
        status = SecItemCopyMatching((__bridge CFDictionaryRef)request, &result);
        CFRelease(keychain);
        if (status == errSecItemNotFound) return 0;
        if (status != errSecSuccess) return status == errSecAuthFailed || status == errSecInteractionNotAllowed ? 2 : 3;
        id objects = CFBridgingRelease(result);
        NSArray *certs = [objects isKindOfClass:NSArray.class] ? objects : @[objects];
        if (certs.count > maximum) return 4;
        for (id item in certs) {
            if (CFGetTypeID((__bridge CFTypeRef)item) != SecCertificateGetTypeID()) return -1;
            NSData *data = CFBridgingRelease(SecCertificateCopyData((__bridge SecCertificateRef)item));
            if (!data || data.length > 65536) return 4;
            if (emit(state, data.bytes, data.length)) return -1;
        }
        return 0;
    }
}
int rss_macos_app_paths(const char *bundle, Emitter emit, void *state) {
    @autoreleasepool {
        NSString *identifier = [NSString stringWithUTF8String:bundle];
        NSArray<NSURL *> *urls = [NSWorkspace.sharedWorkspace URLsForApplicationsWithBundleIdentifier:identifier];
        if (!urls) return 1;
        if (urls.count > 128) return 4;
        for (NSURL *url in urls) {
            if (!url.isFileURL || !url.path.isAbsolutePath) return -1;
            NSData *path = [url.path dataUsingEncoding:NSUTF8StringEncoding];
            if (!path || emit(state, path.bytes, path.length)) return -1;
        }
        // Launch Services only proves its registered set, not an all-disk absence.
        return 0;
    }
}
