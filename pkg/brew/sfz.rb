class Sfz < Formula
  version '0.0.2'
  desc "A simple static file serving command-line tool."
  homepage "https://github.com/weihanglo/sfz"
  head "https://github.com/weihanglo/sfz.git"

  if OS.mac?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "b8ae41685684a4628d97cf353c6ec8e268da70e4540dd05b8d2616b187739ec9"
  elsif OS.linux?
      url "https://github.com/weihanglo/sfz/releases/download/#{version}/sfz-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "51c06901e987d54cf2192fc769af6c937414cca554a0fa23703e1f187aaf0959"
  end

  def install
    bin.install "sfz"
  end

  test do
    system "#{bin}/sfz", "--help"
  end
end
