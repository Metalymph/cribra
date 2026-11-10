#import <Foundation/Foundation.h>

#import <ReactCommon/CxxTurboModuleUtils.h>

#import <CribraReactNative.h>

@interface CribraReactNativeOnLoad : NSObject
@end

@implementation CribraReactNativeOnLoad

+ (void)load
{
  facebook::react::registerCxxModuleToGlobalModuleMap(
      std::string(facebook::react::CribraReactNative::kModuleName),
      [](std::shared_ptr<facebook::react::CallInvoker> jsInvoker) {
        return std::make_shared<facebook::react::CribraReactNative>(
            std::move(jsInvoker));
      });
}

@end
