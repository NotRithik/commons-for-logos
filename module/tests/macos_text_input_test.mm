#import <Foundation/Foundation.h>
#include <cstdio>
extern "C" bool commonsNeedsLegacyAutoFillWorkaround(long, long);
int main()
{
    if (commonsNeedsLegacyAutoFillWorkaround(25, 0) || !commonsNeedsLegacyAutoFillWorkaround(26, 0)
        || !commonsNeedsLegacyAutoFillWorkaround(26, 1) || commonsNeedsLegacyAutoFillWorkaround(26, 2)
        || commonsNeedsLegacyAutoFillWorkaround(27, 0)) return 1;
    @autoreleasepool {
        const auto os = [[NSProcessInfo processInfo] operatingSystemVersion];
        NSDictionary* domain = [[NSUserDefaults standardUserDefaults] volatileDomainForName:NSArgumentDomain];
        id value = domain[@"NSAutoFillHeuristicControllerEnabled"];
        if (commonsNeedsLegacyAutoFillWorkaround(os.majorVersion, os.minorVersion)) {
            if (!value || [value boolValue]) return 2;
        } else if (value) return 3;
        std::puts("PASS: legacy AutoFill preference is limited to macOS 26.0 and 26.1; no persistent preference written");
    }
    return 0;
}
