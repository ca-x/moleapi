# Apple-only QA target for original protoc output; CocoaPods supplies native runtimes.
require 'xcodeproj'

root = File.expand_path(ARGV.fetch(0))
consumer = File.expand_path(ARGV.fetch(1, File.join(root, 'runtime-consumer')))
project_path = File.join(consumer, 'FixtureQA.xcodeproj')
existing = File.directory?(project_path)
project = existing ? Xcodeproj::Project.open(project_path) : Xcodeproj::Project.new(project_path)
target = existing ? project.targets.find { |item| item.name == 'FixtureQA' } : project.new_target(:command_line_tool, 'FixtureQA', :osx, '12.0')
raise 'Native fixture target missing' unless target
unless existing
  target.add_system_framework('Foundation')
  runtime = project.main_group.new_group('Runtime')
  swift_runtime = File.join(consumer, 'LinkRuntime.swift')
  File.write(swift_runtime, "// Link the selected Xcode Swift runtime required by Protobuf.\n")
  [File.join(consumer, 'RuntimeFixture.m'), swift_runtime].each do |path|
    target.source_build_phase.add_file_reference(runtime.new_file(path))
  end
end

generated = project.main_group.children.find { |item| item.isa == 'PBXGroup' && item.name == 'Generated' }
generated ||= project.main_group.new_group('Generated')
generated.files.each do |reference|
  reference.build_files.each(&:remove_from_project)
  reference.remove_from_project
end
sources = Dir.glob(File.join(root, '**', '*.{h,m}')).reject { |path| path.start_with?(consumer + '/') }
sources.each do |path|
  reference = generated.new_file(path)
  next unless File.extname(path) == '.m'
  build_file = target.source_build_phase.add_file_reference(reference)
  # Official GPB messages use MRC; generated service clients and QA consumer use ARC.
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
unless existing
  scheme = Xcodeproj::XCScheme.new
  scheme.add_build_target(target)
  scheme.set_launch_target(target)
  scheme.save_as(project.path, 'FixtureQA', true)
end
