class Sfz < Formula
  version '0.7.1'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "4ece429bb3cb6953c1a6f0b07b50b6220ef219f88a66fb403d820ee933eb0f43"
  elsif OS.linux?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
    sha256 "bbbfa1fafb1fc481d78b292c671c28318108d51f4f5ae54b99a843f52bbcafd1"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
