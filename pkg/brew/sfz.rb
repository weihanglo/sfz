class Sfz < Formula
  version '0.4.0'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "ce00eec5bd917dab6a464fc517fa2bad1d2ce3bd8038ce756dc5547ddc4174b2"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "db964e4762145efa871a6909d5c7f4ad5fbe6659090ce157beff91aca679a72c"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
