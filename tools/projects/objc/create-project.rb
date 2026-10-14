# Apple-only QA target for original protoc output; CocoaPods supplies native runtimes.
require 'xcodeproj'

root = File.expand_path(ARGV.fetch(0))
consumer = File.join(root, 'runtime-consumer')
project = Xcodeproj::Project.new(File.join(consumer, 'FixtureQA.xcodeproj'))
target = project.new_target(:command_line_tool, 'FixtureQA', :osx, '12.0')
target.add_system_framework('Foundation')
sources = Dir.glob(File.join(root, '**', '*.{h,m}')).reject { |p| p.start_with?(consumer + '/') }
sources << File.join(consumer, 'RuntimeFixture.m')
# Protobuf's static archive includes Swift additions and compatibility runtime symbols.
# A Swift compilation unit makes Xcode supply its matching runtime/link search paths.
swift_runtime = File.join(consumer, 'LinkRuntime.swift')
File.write(swift_runtime, "// Link the selected Xcode Swift runtime required by Protobuf.\n")
sources << swift_runtime
sources.each do |path|
  reference = project.main_group.new_file(path)
  next unless ['.m', '.swift'].include?(File.extname(path))
  build_file = target.source_build_phase.add_file_reference(reference)
  # Official GPB generated messages use manual reference counting; the QA client uses ARC.
  build_file.settings = { 'COMPILER_FLAGS' => '-fno-objc-arc' } if path.end_with?('.pbobjc.m')
end
target.build_configurations.each do |config|
  config.build_settings['CLANG_ENABLE_OBJC_ARC'] = 'YES'
  config.build_settings['CLANG_ENABLE_MODULES'] = 'YES'
  config.build_settings['CODE_SIGNING_ALLOWED'] = 'NO'
  config.build_settings['SWIFT_VERSION'] = '5.0'
  config.build_settings['ALWAYS_EMBED_SWIFT_STANDARD_LIBRARIES'] = 'YES'
  config.build_settings['HEADER_SEARCH_PATHS'] = ['$(inherited)', root]
  config.build_settings['GCC_PREPROCESSOR_DEFINITIONS'] = ['$(inherited)', 'GPB_USE_PROTOBUF_FRAMEWORK_IMPORTS=1']
end
project.save
scheme = Xcodeproj::XCScheme.new
scheme.add_build_target(target)
scheme.set_launch_target(target)
scheme.save_as(project.path, 'FixtureQA', true)
