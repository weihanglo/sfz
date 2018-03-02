class Sfz < Formula
  version '0.0.1-beta.1'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "b11517a49e141176fd048b6bae3f778b23fc571d79d258b099d7c15a8a9c3014"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "ab01ec3318d8adb084ebd85bc3f143a1762cb05ad6c9f6e7b10ef8ca7969c745"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "-V"
  end
end
