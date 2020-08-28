class Sfz < Formula
  version '0.1.2'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "20c0bf6272f5854335bea8eb839a6818f69a41e6a7949390a7b014f9ef8ca034"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "e2085abedc4dd85e33268da71426ede27e16493cc0d6e30d28b3b0f40a96ea9b"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
