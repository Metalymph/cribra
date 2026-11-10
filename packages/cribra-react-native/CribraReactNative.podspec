require "json"

package = JSON.parse(File.read(File.join(__dir__, "package.json")))

Pod::Spec.new do |s|
  s.name         = "CribraReactNative"
  s.version      = package["version"]
  s.summary      = package["description"]
  s.homepage     = package["repository"]["url"]
  s.license      = package["license"]
  s.authors      = package["author"]

  s.platforms    = { :ios => min_ios_version_supported }
  s.source       = { :git => package["repository"]["url"], :tag => "v#{s.version}" }

  s.source_files = [
    "ios/**/*.{h,m,mm}",
    "cpp/**/*.{h,cpp}",
    "third_party/cribra/include/**/*.h",
  ]
  s.private_header_files = [
    "cpp/**/*.h",
    "third_party/cribra/include/**/*.h",
  ]

  s.vendored_frameworks = "ios/CribraCAPI.xcframework"

  s.pod_target_xcconfig = {
    "HEADER_SEARCH_PATHS" => [
      "\"$(PODS_TARGET_SRCROOT)/cpp\"",
      "\"$(PODS_TARGET_SRCROOT)/third_party/cribra/include\"",
    ].join(" "),
    "GCC_ENABLE_CPP_EXCEPTIONS" => "YES",
  }

  install_modules_dependencies(s)
end
