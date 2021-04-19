class Sfz < Formula
  version '0.5.0'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "e09c7a493000e8b74428019c01d6a1888e8747147275a8f792ea4a09bc952813"
  elsif OS.linux?
    url "https://github.com/weihanglo/sfz/releases/download/v#{version}/sfz-v#{version}-x86_64-unknown-linux-musl.tar.gz"
    sha256 "eaba9a468ce2b8b241b745af4967780baa32f0fbb842892e17d7660c3c2408f5"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
