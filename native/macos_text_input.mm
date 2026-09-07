// Optional workaround for a macOS 26 AutoFill helper reconnect loop in macOS 26.0/26.1. The preference was removed in 26.2.
// A current system must not be described as fixed by this legacy setting. It writes ONLY this process's volatile
// argument domain, never a persistent user preference or a keychain entry.
#import <Foundation/Foundation.h>
#include <cstdlib>
#include <cstring>

extern "C" bool commonsNeedsLegacyAutoFillWorkaround(long major, long minor)
{
    return major == 26 && minor < 2;
}

__attribute__((constructor)) static void commonsConfigureNativeTextInput()
{
    const char* enabled = std::getenv("COMMONS_DISABLE_NATIVE_AUTOFILL");
    if (!enabled || std::strcmp(enabled, "1") != 0) return;
    @autoreleasepool {
        const NSOperatingSystemVersion os = [[NSProcessInfo processInfo] operatingSystemVersion];
        if (!commonsNeedsLegacyAutoFillWorkaround(os.majorVersion, os.minorVersion)) return;
        NSUserDefaults* defaults = [NSUserDefaults standardUserDefaults];
        NSMutableDictionary* domain = [[defaults volatileDomainForName:NSArgumentDomain] mutableCopy];
        if (!domain) domain = [NSMutableDictionary dictionary];
        domain[@"NSAutoFillHeuristicControllerEnabled"] = @NO;
        [defaults setVolatileDomain:domain forName:NSArgumentDomain];
    }
}
