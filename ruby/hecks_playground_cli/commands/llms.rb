HecksPlayground::CLI.handle(:llms) do |inv|
  domain = resolve_domain_option
  next unless domain

  puts HecksPlayground::LlmsGenerator.new(domain).generate
end
