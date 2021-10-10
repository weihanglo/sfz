class Sfz < Formula
  version '0.6.2'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "a35d88ea6ccc6973759859504bb86314e0eff0c5b8c2a3901570febdd1043ee9"
  elsif OS.linux?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
    sha256 "7979a1cb74a4961b9a5388f3af4e9fd0093a9ac7f8f5907de85508ac615fd936"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
