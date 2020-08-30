class Sfz < Formula
  version '0.2.0'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "513b03866e18f1bbf9b7df8014c4932bda563816cf9980c1065f08ea74334659"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "7d1b04a3a3f6f1503eb9ad87ace2dc25b0adf76b0a25b969cff05145f09dcc1c"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
