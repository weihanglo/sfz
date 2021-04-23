class Sfz < Formula
  version '0.6.0'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "45347de08a8608e062b4f0fc2a38a71fe99c6a10ebce00b2baa26f7510827c78"
  elsif OS.linux?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
    sha256 "298bcc7eea356e8a35a4293f2c48be69a5bdec532a3083b3c57b3e6dedc71619"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
