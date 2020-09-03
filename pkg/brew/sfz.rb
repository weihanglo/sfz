class Sfz < Formula
  version '0.2.1'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0a7980657efb80e477a8f994b0eb54b9011a19657bff73080c3d95b64d1f0e4f"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "bf36c49b02578b241862a97bb9da92f7ad984f8935f80ee8baf947d1eafba110"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
