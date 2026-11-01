// Go net/http (Go's own crypto/tls), HTTP/1.1, key in a header.
package main

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"strings"
	"time"
)

func main() {
	var cfg struct {
		Ca    string         `json:"ca"`
		Ports map[string]int `json:"ports"`
	}
	b, _ := os.ReadFile("matrix.json")
	json.Unmarshal(b, &cfg)
	pem, _ := os.ReadFile(cfg.Ca)
	pool := x509.NewCertPool()
	pool.AppendCertsFromPEM(pem)
	for _, name := range []string{"api.stripe.test", "evil.test"} {
		port := cfg.Ports[name]
		tr := &http.Transport{
			TLSClientConfig: &tls.Config{RootCAs: pool},
			DialContext: func(ctx context.Context, n, _ string) (net.Conn, error) {
				return (&net.Dialer{}).DialContext(ctx, n, fmt.Sprintf("127.0.0.1:%d", port))
			},
		}
		req, _ := http.NewRequest("POST", fmt.Sprintf("https://%s:%d/v1/charges", name, port), strings.NewReader("amount=100"))
		req.Header.Set("Authorization", "Bearer "+os.Getenv("STRIPE_API_KEY"))
		t := time.Now()
		r, err := (&http.Client{Transport: tr}).Do(req)
		if err != nil {
			fmt.Println(name, "refused", err)
			continue
		}
		io.Copy(io.Discard, r.Body)
		fmt.Printf("%s ok TIME %.3f\n", name, float64(time.Since(t).Microseconds())/1000)
	}
}
