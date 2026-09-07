#import <Foundation/Foundation.h>
#include <cstdio>
extern "C" bool astraNeedsLegacyAutoFillWorkaround(long, long);
int main()
{
    if (astraNeedsLegacyAutoFillWorkaround(25, 0) || !astraNeedsLegacyAutoFillWorkaround(26, 0)
        || !astraNeedsLegacyAutoFillWorkaround(26, 1) || astraNeedsLegacyAutoFillWorkaround(26, 2)
        || astraNeedsLegacyAutoFillWorkaround(27, 0)) return 1;
    @autoreleasepool {
        const auto os = [[NSProcessInfo processInfo] operatingSystemVersion];
        NSDictionary* domain = [[NSUserDefaults standardUserDefaults] volatileDomainForName:NSArgumentDomain];
        id value = domain[@"NSAutoFillHeuristicControllerEnabled"];
        if (astraNeedsLegacyAutoFillWorkaround(os.majorVersion, os.minorVersion)) {
            if (!value || [value boolValue]) return 2;
        } else if (value) return 3;
        std::puts("PASS: legacy AutoFill preference is limited to macOS 26.0 and 26.1; no persistent preference written");
    }
    return 0;
}
