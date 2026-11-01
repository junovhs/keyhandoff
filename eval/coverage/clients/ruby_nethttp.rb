# Ruby Net::HTTP (OpenSSL through the openssl extension), HTTP/1.1, key in a header.
require "json"; require "net/http"; require "openssl"
cfg = JSON.parse(File.read("matrix.json"))
key = ENV.fetch("STRIPE_API_KEY")
["api.stripe.test", "evil.test"].each do |name|
  t = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  begin
    http = Net::HTTP.new(name, cfg["ports"][name])
    http.ipaddr = "127.0.0.1"
    http.use_ssl = true
    http.ca_file = cfg["ca"]
    r = http.post("/v1/charges", "amount=100", { "Authorization" => "Bearer #{key}" })
    puts "#{name} ok #{r.code} TIME #{format('%.3f', (Process.clock_gettime(Process::CLOCK_MONOTONIC) - t) * 1000)}"
  rescue StandardError => e
    puts "#{name} refused #{e.class}"
  end
end
