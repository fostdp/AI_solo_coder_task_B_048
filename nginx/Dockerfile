FROM nginx:alpine

RUN apk add --no-cache gettext nginx-mod-http-brotli

ENV BACKEND_URL=http://backend:8080

COPY nginx/nginx.conf.template /etc/nginx/nginx.conf.template

COPY frontend /usr/share/nginx/html

EXPOSE 80

CMD ["/bin/sh", "-c", "envsubst '${BACKEND_URL}' < /etc/nginx/nginx.conf.template > /etc/nginx/nginx.conf && nginx -g 'daemon off;'"]
