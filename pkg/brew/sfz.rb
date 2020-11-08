class Sfz < Formula
  version '0.3.0'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "baf2ec817b186668a0d8df21f15da09dc80fcfd9810f197c1814be9681cd6ac9"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "dadf2d8d56c1c5b5b66e255bac718912f80b269598f997b127cba76e9abcd81e"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
