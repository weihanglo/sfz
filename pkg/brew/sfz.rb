class Sfz < Formula
  version '0.0.4'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "a24f6742eacc307e09391686ba4972873c61fa94085e295fadff16f1bc3cc392"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "50ab4cd2f90ab864b852c13f1af330059dde2950783971641a8944cbf3e918d6"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
